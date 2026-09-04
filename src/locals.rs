use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use anyhow::Context;
use bytes_str::BytesStr;
use crossbeam_channel::{Receiver, RecvError, RecvTimeoutError, Sender, unbounded};
use itertools::Itertools;
use lsp_types::Uri;

use crate::{
    analyze::{
        IDENTIFIER_KIND, TYPE_IDENTIFIER_KIND, locals, parse,
        types::{Ident, SymbolKind},
    },
    text::{
        Change, Position, PositionEncoding, PositionRange, buffer::TextBuffer, flat_rope::FlatRope,
    },
};

pub struct LocalsStore {
    encoding: PositionEncoding,
    documents: BTreeMap<Uri, Arc<Mutex<Document>>>,
    background_queue_tx: Sender<BackgroundAction>,
}

pub struct Document {
    source: TextBuffer,
    encoding: PositionEncoding,
    tree: tree_sitter::Tree,
    locals: locals::Locals,
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
    pub fn new(encoding: PositionEncoding) -> Self {
        let (background_queue_tx, background_queue_rx) = unbounded();

        std::thread::spawn(|| background_thread(background_queue_rx));

        Self {
            encoding,
            documents: BTreeMap::new(),
            background_queue_tx,
        }
    }

    pub fn load(&mut self, uri: Uri, source: String) -> anyhow::Result<()> {
        let document = Document::parse(source, self.encoding)?;

        let document = Arc::new(Mutex::new(document));
        self.documents.insert(uri.clone(), document.clone());

        self.background_queue_tx
            .send(BackgroundAction::Load { uri, document })?;

        Ok(())
    }

    pub fn update(&mut self, uri: Uri, changes: Vec<Change>) -> anyhow::Result<()> {
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
    fn parse(source: String, encoding: PositionEncoding) -> anyhow::Result<Self> {
        let tree = parse(source.as_bytes(), None)?;

        Ok(Self {
            source: TextBuffer::from(source),
            encoding,
            tree,
            // Locals are loaded on a background thread.
            locals: Default::default(),
        })
    }

    fn update(&mut self, changes: Vec<Change>) -> anyhow::Result<()> {
        let mut incremental_changes = Vec::new();
        let mut complete_change = None;
        for change in changes {
            match change {
                Change::Complete(new_text) => {
                    complete_change = Some(new_text);
                    // All changes before this point would have been overwritten.
                    incremental_changes.clear();
                }
                Change::Incremental(incremental_change) => {
                    incremental_changes.push(incremental_change)
                }
            }
        }

        let use_old_tree;
        if let Some(new_text) = complete_change {
            self.source = TextBuffer::from(new_text);
            use_old_tree = false;
        } else {
            use_old_tree = true;
        }

        if !incremental_changes.is_empty() {
            let mut rope = FlatRope::from(std::mem::take(&mut self.source));

            for change in incremental_changes {
                let edit = rope.edit(change, self.encoding)?;

                if use_old_tree {
                    self.tree.edit(&edit);
                }
            }

            self.source = rope.freeze();
        }

        let old_tree = use_old_tree.then_some(&self.tree);
        self.tree = parse(self.source.as_bytes(), old_tree)?;

        Ok(())
    }

    pub fn ident_at(&self, pos: Position) -> Option<tree_sitter::Node<'_>> {
        let (byte, _) = pos.to_ts(&self.source, self.encoding);

        let node = self
            .tree
            .root_node()
            .descendant_for_byte_range(byte, byte)?;

        if let IDENTIFIER_KIND | TYPE_IDENTIFIER_KIND = node.kind_id() {
            Some(node)
        } else {
            None
        }
    }

    /// Returns the locations of the references we found and a boolean
    /// to indicate if the symbol is a local variable.
    pub fn find_references(&self, ident: tree_sitter::Node) -> (Vec<PositionRange>, bool) {
        let ident = Ident::from_node(ident, &self.source.to_bytes(), self.encoding);

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

    pub fn find_definitions(&self, ident: tree_sitter::Node) -> Vec<PositionRange> {
        let ident = Ident::from_node(ident, &self.source.to_bytes(), self.encoding);

        let definitions = self.locals.definitions(ident);

        definitions.into_iter().map(|d| d.name).collect()
    }

    pub fn text_for<'a>(&'a self, node: tree_sitter::Node) -> BytesStr {
        self.source.slice(node.byte_range())
    }

    pub fn completions(&self, pos: Position) -> Vec<(&[u8], SymbolKind)> {
        self.locals
            .definitions
            .iter()
            .flat_map(move |(name, defs)| {
                defs.iter()
                    .filter(move |def| def.scope.contains_pos(pos))
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
    let (source, tree, encoding) = {
        let document = document.lock().expect("failed to lock document");

        (
            // Ropes are cheap to copy.
            document.source.to_bytes_str(),
            // Trees are cheap to copy.
            document.tree.clone(),
            document.encoding,
        )
    };

    let locals = locals::analyze(tree.root_node(), &source, encoding);

    {
        let mut document = document.lock().expect("failed to lock document");
        document.locals = locals;
    }

    Ok(())
}
