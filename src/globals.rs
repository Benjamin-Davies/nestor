use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Context;
use bytes::Bytes;
use crossbeam_channel::{Receiver, Sender, unbounded};
use itertools::Itertools;
use lsp_types::{Uri, WorkspaceFolder};

use crate::analyze::{dirs, types::Range};

const GIT_DIR_NAME: &str = ".git";

pub struct GlobalsStore {
    workspace_folders: Vec<PathBuf>,
    roots: Vec<Root>,
    load_queue_tx: Sender<(PathBuf, RootDataHandle)>,
}

pub struct Symbol {
    pub name: Bytes,
    pub uri: Uri,
    pub range: Range,
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

impl GlobalsStore {
    pub fn new() -> Self {
        let (load_queue_tx, load_queue_rx) = unbounded();

        std::thread::spawn(|| load_files(load_queue_rx));

        Self {
            workspace_folders: Vec::new(),
            roots: Vec::new(),
            load_queue_tx,
        }
    }

    pub fn set_workspace_folders(&mut self, folders: &[WorkspaceFolder]) {
        self.workspace_folders = folders
            .iter()
            .filter_map(|f| file_uri_to_path(&f.uri))
            .collect_vec();
    }

    pub fn add_workspace_folder(&mut self, folder: &WorkspaceFolder) {
        let Some(path) = file_uri_to_path(&folder.uri) else {
            return;
        };

        self.workspace_folders.push(path);
    }

    pub fn remove_workspace_folder(&mut self, folder: &WorkspaceFolder) {
        let Some(path) = file_uri_to_path(&folder.uri) else {
            return;
        };

        self.workspace_folders.retain(|p| p != &path);
    }

    /// Called when a file is opened to indicate that the file's surroundings
    /// should be loaded.
    pub fn seed(&mut self, uri: &Uri) -> anyhow::Result<()> {
        let Some(path) = file_uri_to_path(uri) else {
            return Ok(());
        };

        if let Some(root) = self.roots.iter_mut().find(|r| path.starts_with(&r.path)) {
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
                .send((root_path.clone(), root_data.clone()))?;

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
    pub fn unseed(&mut self, uri: &Uri) {
        let Some(path) = file_uri_to_path(uri) else {
            return;
        };

        self.roots.retain_mut(|root| {
            if path.starts_with(&root.path) {
                root.open_file_count = root.open_file_count.saturating_sub(1);
            }
            root.open_file_count != 0
        });
    }

    pub fn find_definitions(&self, name: &[u8]) -> Vec<Symbol> {
        self.find_symbol(name, true)
    }

    pub fn find_references(&self, name: &[u8]) -> Vec<Symbol> {
        self.find_symbol(name, false)
    }

    fn find_symbol(&self, name: &[u8], is_definition: bool) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        for root in &self.roots {
            let root_data = root.data.lock().expect("failed to lock root data");
            let root_symbols = root_data
                .find_symbol(name)
                .iter()
                .inspect(|s| {
                    // This should always be the case, but check just in case.
                    debug_assert_eq!(s.name, name);
                })
                .filter(|s| s.is_definition == is_definition)
                .filter_map(|s| {
                    s.try_into()
                        .inspect_err(|err| tracing::warn!("Error finding symbol: {err}"))
                        .ok()
                });
            symbols.extend(root_symbols);
        }

        symbols
    }
}

impl RootData {
    fn find_symbol(&self, name: &[u8]) -> &[dirs::Symbol] {
        let start = self
            .symbols
            .binary_search_by(|symbol| match <[u8]>::cmp(&symbol.name, name) {
                Ordering::Less => Ordering::Less,
                Ordering::Greater | Ordering::Equal => Ordering::Greater,
            })
            .expect_err("closure should never return eq");
        let end = self
            .symbols
            .binary_search_by(|symbol| match <[u8]>::cmp(&symbol.name, name) {
                Ordering::Less | Ordering::Equal => Ordering::Less,
                Ordering::Greater => Ordering::Greater,
            })
            .expect_err("closure should never return eq");

        &self.symbols[start..end]
    }
}

impl TryFrom<&dirs::Symbol> for Symbol {
    type Error = anyhow::Error;

    fn try_from(symbol: &dirs::Symbol) -> anyhow::Result<Self> {
        Ok(Symbol {
            name: symbol.name.clone(),
            uri: path_to_file_uri(&symbol.path).context("failed to convert path to file uri")?,
            range: symbol.range,
        })
    }
}

// This function receives weak node handles so that we can discard them if all
// files are closed.
fn load_files(load_queue_rx: Receiver<(PathBuf, RootDataHandle)>) {
    let mut stack = Vec::new();
    while let Some((path, root_data)) = stack.pop().or_else(|| load_queue_rx.recv().ok()) {
        let mut symbols = dirs::analyze(&path);
        tracing::info!("Analyzed {path:?}: {}", symbols.len());

        symbols.sort();
        let mut root_data = root_data.lock().expect("failed to lock root data");
        root_data.symbols = symbols;
    }
}

fn file_uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if uri.scheme().map(|s| s.as_str()) != Some("file") {
        return None;
    }

    Some(uri.path().as_str().into())
}

fn path_to_file_uri(path: &Path) -> Option<Uri> {
    let s = path.as_os_str().to_str()?;
    format!("file://{s}").parse().ok()
}

fn find_git_dir<'a>(mut path: &'a Path, base: &Path) -> Option<&'a Path> {
    while path.starts_with(base) {
        if path.join(GIT_DIR_NAME).is_dir() {
            return Some(path);
        }

        path = path.parent()?;
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lsp_types::Uri;

    use crate::globals::{file_uri_to_path, path_to_file_uri};

    #[test]
    fn convert_file_uri_to_path() {
        assert_eq!(
            file_uri_to_path(&"http://example.com/".parse::<Uri>().unwrap()),
            None,
        );
        assert_eq!(
            file_uri_to_path(&"file:///".parse::<Uri>().unwrap()),
            Some("/".into()),
        );
        assert_eq!(
            file_uri_to_path(&"file:///home/bend/foo.txt".parse::<Uri>().unwrap()),
            Some("/home/bend/foo.txt".into()),
        );
    }

    #[test]
    fn convert_path_to_file_uri() {
        assert_eq!(
            path_to_file_uri(Path::new("/")),
            Some("file:///".parse::<Uri>().unwrap()),
        );
        assert_eq!(
            path_to_file_uri(Path::new("/home/bend/foo.txt")),
            Some("file:///home/bend/foo.txt".parse::<Uri>().unwrap()),
        );
    }
}
