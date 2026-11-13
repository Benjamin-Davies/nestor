use std::{
    collections::{BTreeMap, btree_map},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Context;
use bytes::Bytes;
use crossbeam_channel::{Receiver, Sender, unbounded};
use itertools::Itertools;
use url::Url;

use crate::{
    analyze::{
        dirs, hover, parse,
        types::{Ident, SymbolKind},
    },
    text::{PositionEncoding, PositionRange, buffer::TextBuffer, markdown::Markdown},
    utils::binary_search_range_by_key,
};

pub struct GlobalsStore {
    workspace_folders: Vec<PathBuf>,
    roots: Vec<Root>,
    load_queue_tx: Sender<LoadCommand>,
}

pub struct Symbol {
    pub name: Bytes,
    pub uri: Url,
    pub range: PositionRange,
    pub kind: SymbolKind,
}

/// The entire workspace isn't loaded into memory all at once. Instead, when a
/// file is opened, we find the repo that it is in and load that. We are careful
/// to never load files from outside the workspace though.
struct Root {
    path: PathBuf,
    /// The number of files that the user has open that from inside this root.
    open_file_count: u64,
    data: RootDataHandle,
}

type RootDataHandle = Arc<Mutex<RootData>>;

#[derive(Default)]
struct RootData {
    /// A sorted list of symbols in this root.
    symbols: Vec<dirs::Symbol>,
}

enum LoadCommand {
    LoadRoot(PathBuf, RootDataHandle),
    RefreshFile(PathBuf, Bytes, RootDataHandle),
}

impl GlobalsStore {
    pub fn new(encoding: PositionEncoding) -> Self {
        let (load_queue_tx, load_queue_rx) = unbounded();

        std::thread::spawn(move || load_files(load_queue_rx, encoding));

        Self {
            workspace_folders: Vec::new(),
            roots: Vec::new(),
            load_queue_tx,
        }
    }

    pub fn set_workspace_folders(&mut self, uris: impl IntoIterator<Item = Url>) {
        self.workspace_folders = uris
            .into_iter()
            .filter_map(|u| u.to_file_path().ok())
            .collect_vec();
    }

    pub fn add_workspace_folder(&mut self, uri: &Url) {
        let Ok(path) = uri.to_file_path() else {
            return;
        };

        self.workspace_folders.push(path);
    }

    pub fn remove_workspace_folder(&mut self, uri: &Url) {
        let Ok(path) = uri.to_file_path() else {
            return;
        };

        self.workspace_folders.retain(|p| p != &path);
    }

    /// Called when a file is opened to indicate that the file's surroundings
    /// should be loaded.
    pub fn seed(&mut self, uri: &Url) -> anyhow::Result<()> {
        let Ok(path) = uri.to_file_path() else {
            return Ok(());
        };

        if let Some(root) = self.root_for_path(&path) {
            root.open_file_count += 1;

            // TODO: Load the file into the root if we haven't seen it before

            return Ok(());
        }

        if let Some(workspace_folder) = self.workspace_folders.iter().find(|f| path.starts_with(f))
        {
            let root_path = find_git_dir(&path, workspace_folder)
                .unwrap_or(workspace_folder)
                .to_owned();

            // TODO: check that none of the other roots are inside this root

            let root_data = RootDataHandle::default();

            self.load_queue_tx
                .send(LoadCommand::LoadRoot(root_path.clone(), root_data.clone()))?;

            let root = Root {
                path: root_path,
                open_file_count: 1,
                data: root_data,
            };
            self.roots.push(root);
        }

        Ok(())
    }

    /// Called when a file is closed to indicate that the file's surroundings
    /// may no longer be relevant.
    pub fn unseed(&mut self, uri: &Url) {
        let Ok(path) = uri.to_file_path() else {
            return;
        };

        self.roots.retain_mut(|root| {
            if path.starts_with(&root.path) {
                root.open_file_count = root.open_file_count.saturating_sub(1);
            }
            root.open_file_count != 0
        });
    }

    /// Called when a file is saved to refresh its symbols.
    pub fn refresh(&mut self, uri: &Url, source: Bytes) -> anyhow::Result<()> {
        let Ok(path) = uri.to_file_path() else {
            return Ok(());
        };

        let Some(root) = self.root_for_path(&path) else {
            tracing::info!("{path:?} is not in a root");
            return Ok(());
        };
        let root_data = root.data.clone();

        self.load_queue_tx
            .send(LoadCommand::RefreshFile(path.to_owned(), source, root_data))?;

        Ok(())
    }

    fn root_for_path(&mut self, path: &Path) -> Option<&mut Root> {
        self.roots.iter_mut().find(|r| path.starts_with(&r.path))
    }

    pub fn find_definitions(&self, ident: &Ident) -> Vec<Symbol> {
        self.find_symbol(ident, true)
    }

    pub fn find_references(&self, ident: &Ident) -> Vec<Symbol> {
        self.find_symbol(ident, false)
    }

    fn find_symbol(&self, ident: &Ident, is_definition: bool) -> Vec<Symbol> {
        let class = ident.kind.class();
        let mut symbols = Vec::new();
        for root in &self.roots {
            let root_data = root.data.lock().expect("failed to lock root data");
            let root_symbols = root_data
                .find_symbol(&ident.bytes)
                .iter()
                .inspect(|s| {
                    // This should always be the case, but check just in case.
                    debug_assert_eq!(s.name, ident.bytes);
                })
                .filter(|s| s.kind.class() == class && s.is_definition == is_definition)
                .map(Symbol::from);
            symbols.extend(root_symbols);
        }

        symbols
    }

    pub fn all_definitions(&self) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        for root in &self.roots {
            let root_data = root.data.lock().expect("failed to lock root data");
            let root_symbols = root_data
                .symbols
                .iter()
                .filter(|s| s.is_definition)
                .map(Symbol::from);
            symbols.extend(root_symbols);
        }

        symbols
    }
}

impl RootData {
    fn find_symbol(&self, name: &[u8]) -> &[dirs::Symbol] {
        let range = binary_search_range_by_key(&self.symbols, &name, |symbol| &symbol.name);

        &self.symbols[range]
    }
}

impl From<&dirs::Symbol> for Symbol {
    fn from(symbol: &dirs::Symbol) -> Self {
        Symbol {
            name: symbol.name.clone(),
            uri: Url::from_file_path(&symbol.path).expect("expected absolute file path"),
            range: symbol.range,
            kind: symbol.kind,
        }
    }
}

// This function receives weak node handles so that we can discard them if all
// files are closed.
fn load_files(load_queue_rx: Receiver<LoadCommand>, encoding: PositionEncoding) {
    while let Ok(load_command) = load_queue_rx.recv() {
        match load_command {
            LoadCommand::LoadRoot(path, root_data) => {
                let start = Instant::now();

                let mut symbols = dirs::analyze(&path, encoding);

                let stop = Instant::now();
                tracing::info!(
                    "Analyzed {path:?}: {} symbols in {:.3} seconds",
                    symbols.len(),
                    (stop - start).as_secs_f32()
                );

                symbols.sort();
                let mut root_data = root_data.lock().expect("failed to lock root data");
                root_data.symbols = symbols;
            }
            LoadCommand::RefreshFile(path, source, root_data) => {
                let symbols = dirs::analyze_file(&path, source, encoding).unwrap_or_else(|err| {
                    tracing::error!("Error analyzing {path:?}: {err}");
                    Vec::new()
                });
                tracing::info!("Re-analyzed {path:?}: {}", symbols.len());

                let mut root_data = root_data.lock().expect("failed to lock root data");
                root_data.symbols.retain(|s| *s.path != path);
                root_data.symbols.extend(symbols);
                root_data.symbols.sort();
            }
        }
    }
}

#[derive(Default)]
pub struct GlobalsHoverCache {
    cache: BTreeMap<Url, CacheEntry>,
}

struct CacheEntry {
    source: TextBuffer,
    tree: tree_sitter::Tree,
}

impl GlobalsHoverCache {
    fn load(&mut self, uri: Url) -> anyhow::Result<btree_map::OccupiedEntry<'_, Url, CacheEntry>> {
        match self.cache.entry(uri) {
            btree_map::Entry::Vacant(entry) => {
                let uri = entry.key();
                tracing::info!("Reading {uri} for hover");

                let path = uri
                    .to_file_path()
                    .ok()
                    .with_context(|| format!("{uri} is not a file URL"))?;
                let source = fs::read(&path).with_context(|| format!("Failed to read {path:?}"))?;

                let source = TextBuffer::from(String::from_utf8_lossy_owned(source));
                let tree = parse(source.as_bytes(), None)?;

                let entry = entry.insert_entry(CacheEntry { source, tree });
                Ok(entry)
            }
            btree_map::Entry::Occupied(entry) => Ok(entry),
        }
    }

    pub fn hover(
        &mut self,
        uri: Url,
        definition: PositionRange,
        encoding: PositionEncoding,
    ) -> anyhow::Result<Markdown> {
        let cache_entry = self.load(uri)?;
        let file = cache_entry.get();

        let snippet = hover::hover(&file.source, encoding, &file.tree, definition);

        Ok(snippet)
    }
}

const GIT_DIR_NAME: &str = ".git";

fn find_git_dir<'a>(mut path: &'a Path, base: &Path) -> Option<&'a Path> {
    while path.starts_with(base) {
        if path.join(GIT_DIR_NAME).is_dir() {
            return Some(path);
        }

        path = path.parent()?;
    }

    None
}
