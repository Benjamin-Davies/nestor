use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use anyhow::Context;
use bytes::Bytes;
use crossbeam_channel::{Receiver, RecvError, RecvTimeoutError, Sender, unbounded};
use itertools::Itertools;
use lsp_types::Uri;
use ropey::Rope;
use tree_sitter::InputEdit;

use crate::analyze::{
    IDENTIFIER_KIND, TYPE_IDENTIFIER_KIND, locals, parse, parse_rope,
    types::{Ident, Point, Range, SymbolKind},
};

pub struct LocalsStore {
    documents: BTreeMap<Uri, Arc<Mutex<Document>>>,
    background_queue_tx: Sender<BackgroundAction>,
}

pub struct Document {
    source: Rope,
    tree: tree_sitter::Tree,
    locals: locals::Locals,
}

pub struct DocumentChange {
    pub old_range: Range,
    pub new_text: String,
}

enum BackgroundAction {
    Load {
        uri: Uri,
        document: Arc<Mutex<Document>>,
    },
    Update {
        uri: Uri,
    },
    Unload {
        uri: Uri,
    },
}

const UPDATE_DEBOUNCE: Duration = Duration::from_secs(1);

impl LocalsStore {
    pub fn new() -> Self {
        let (background_queue_tx, background_queue_rx) = unbounded();

        std::thread::spawn(|| background_thread(background_queue_rx));

        Self {
            documents: BTreeMap::new(),
            background_queue_tx,
        }
    }

    pub fn load(&mut self, uri: Uri, source: String) -> anyhow::Result<()> {
        let document = Document::parse(source)?;

        let document = Arc::new(Mutex::new(document));
        self.documents.insert(uri.clone(), document.clone());

        self.background_queue_tx
            .send(BackgroundAction::Load { uri, document })?;

        Ok(())
    }

    pub fn update(&mut self, uri: Uri, changes: Vec<DocumentChange>) -> anyhow::Result<()> {
        {
            let mut document = self.document(&uri)?;
            document.update(changes)?;
        }

        self.background_queue_tx
            .send(BackgroundAction::Update { uri })?;

        Ok(())
    }

    pub fn unload(&mut self, uri: Uri) -> anyhow::Result<()> {
        self.documents.remove(&uri);

        self.background_queue_tx
            .send(BackgroundAction::Unload { uri })?;

        Ok(())
    }

    pub fn document<'a>(&'a self, uri: &Uri) -> anyhow::Result<MutexGuard<'a, Document>> {
        let document = self
            .documents
            .get(uri)
            .with_context(|| format!("Document not open: {}", uri.as_str()))?;

        Ok(document.lock().expect("failed to lock document"))
    }
}

impl Document {
    fn parse(source: String) -> anyhow::Result<Self> {
        let tree = parse(source.as_bytes())?;

        Ok(Self {
            source: Rope::from(source),
            tree,
            // Locals are loaded on a background thread.
            locals: Default::default(),
        })
    }

    fn update(&mut self, changes: Vec<DocumentChange>) -> anyhow::Result<()> {
        for DocumentChange {
            old_range,
            new_text,
        } in changes
        {
            // TODO: Consider non-ASCII chars
            let start_index = self.source.line_to_char(old_range.start.row as usize)
                + old_range.start.column as usize;
            let end_index = self.source.line_to_char(old_range.end.row as usize)
                + old_range.end.column as usize;

            self.source.remove(start_index..end_index);
            self.source.insert(start_index, &new_text);

            let new_end_char = start_index + new_text.chars().count();
            let new_end_line = self.source.char_to_line(new_end_char);
            let new_end_line_start = self.source.line_to_char(new_end_line);
            let new_end_column = new_end_char - new_end_line_start;
            let new_end_position = tree_sitter::Point {
                row: new_end_line,
                column: new_end_column,
            };

            self.tree.edit(&InputEdit {
                start_byte: start_index,
                old_end_byte: end_index,
                new_end_byte: start_index + new_text.len(),
                start_position: old_range.start.into(),
                old_end_position: old_range.end.into(),
                new_end_position,
            });
        }

        self.tree = parse_rope(&self.source, Some(&self.tree))?;

        Ok(())
    }

    pub fn ident_at(&self, point: Point) -> Option<tree_sitter::Node<'_>> {
        let ts_point = tree_sitter::Point::from(point);

        let node = self
            .tree
            .root_node()
            .descendant_for_point_range(ts_point, ts_point)?;

        if let IDENTIFIER_KIND | TYPE_IDENTIFIER_KIND = node.kind_id() {
            Some(node)
        } else {
            None
        }
    }

    /// Returns the locations of the references we found and a boolean
    /// to indicate if the symbol is a local variable.
    pub fn find_references(&self, ident: tree_sitter::Node) -> (Vec<Range>, bool) {
        let ident = Ident::from_node_rope(ident, &self.source);

        let symbols = self.locals.symbols.get(&ident.bytes);

        let mut definitions = self.locals.definitions(ident);
        definitions.retain(|d| d.kind != SymbolKind::Function);
        if !definitions.is_empty() {
            // If there are local vars that match, assume that the user
            // wants references to those vars.
            let matches = symbols
                .into_iter()
                .flatten()
                .copied()
                .filter(|&s| definitions.iter().any(|d| d.scope.contains(s)))
                .collect();
            (matches, true)
        } else {
            let matches = symbols.cloned().unwrap_or_default();
            (matches, false)
        }
    }

    pub fn find_definitions(&self, ident: tree_sitter::Node) -> Vec<Range> {
        let ident = Ident::from_node_rope(ident, &self.source);

        let definitions = self.locals.definitions(ident);

        definitions.into_iter().map(|d| d.name).collect()
    }

    pub fn bytes_for<'a>(&'a self, node: tree_sitter::Node) -> Vec<u8> {
        self.source
            .byte_slice(node.byte_range())
            .to_string()
            .into_bytes()
    }

    pub fn completions(&self, point: Point) -> Vec<(&[u8], SymbolKind)> {
        self.locals
            .definitions
            .iter()
            .flat_map(move |(name, defs)| {
                defs.iter()
                    .filter(move |def| def.scope.contains_point(point))
                    .map(|def| (name as &[_], def.kind))
            })
            .collect_vec()
    }
}

fn background_thread(actions: Receiver<BackgroundAction>) {
    let mut documents = BTreeMap::new();

    loop {
        // The time after which we will apply document updates, if no
        // more updates occur.
        let mut debounce_deadline = None;
        // This could be a set type but it would be nice to preserve
        // the order that updates are applied. Besides, there shouldn't
        // be too many concurrent updates.
        let mut updates = Vec::new();

        loop {
            let action = match recv_deadline_maybe(&actions, debounce_deadline) {
                Ok(action) => action,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            };

            match action {
                BackgroundAction::Load { uri, document } => {
                    if let Err(err) = analyze_document(&document) {
                        tracing::error!("Error during background load: {err}");
                        continue;
                    }

                    tracing::info!("Loaded {}", uri.as_str());

                    documents.insert(uri, document);
                }
                BackgroundAction::Update { uri } => {
                    if !updates.contains(&uri) {
                        updates.push(uri);
                    }

                    debounce_deadline = Some(Instant::now() + UPDATE_DEBOUNCE);
                }
                BackgroundAction::Unload { uri } => {
                    documents.remove(&uri);
                    updates.retain(|u| u != &uri);
                }
            }
        }

        for uri in updates {
            let Some(document) = documents.get(&uri) else {
                continue;
            };

            if let Err(err) = analyze_document(document) {
                tracing::error!("Error during background update: {err}");
                continue;
            }

            tracing::info!("Updated {}", uri.as_str());
        }
    }
}

fn recv_deadline_maybe<T>(
    receiver: &Receiver<T>,
    deadline: Option<Instant>,
) -> Result<T, RecvTimeoutError> {
    if let Some(deadline) = deadline {
        receiver.recv_deadline(deadline)
    } else {
        receiver
            .recv()
            .map_err(|RecvError| RecvTimeoutError::Disconnected)
    }
}

fn analyze_document(document: &Mutex<Document>) -> anyhow::Result<()> {
    let (source, tree);
    {
        let document = document.lock().expect("failed to lock document");
        source = Bytes::from(document.source.to_string());
        // Trees are cheap to copy.
        tree = document.tree.clone();
    }

    let locals = locals::analyze(tree.root_node(), &source);

    {
        let mut document = document.lock().expect("failed to lock document");
        document.locals = locals;
    }

    Ok(())
}
