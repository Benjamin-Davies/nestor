use anyhow::Context;
use itertools::Itertools;
use lsp_types::{Hover, HoverContents, LanguageString, MarkedString};

use crate::{
    analyze::{KEYWORDS, types::SymbolKind},
    globals::{GlobalsHoverCache, GlobalsStore},
    locals::LocalsStore,
    text::{Change, Position, PositionEncoding, PositionRange},
    utils::Uri,
};

pub struct Workspace {
    encoding: PositionEncoding,
    locals_store: LocalsStore,
    globals_store: GlobalsStore,
}

impl Workspace {
    pub fn new<'a>(encoding: PositionEncoding, folders: impl IntoIterator<Item = Uri>) -> Self {
        let locals_store = LocalsStore::new(encoding);

        let mut globals_store = GlobalsStore::new(encoding);
        globals_store.set_workspace_folders(folders);

        Self {
            encoding,
            locals_store,
            globals_store,
        }
    }

    pub fn add_folder(&mut self, uri: &Uri) {
        self.globals_store.add_workspace_folder(uri);
    }

    pub fn remove_folder(&mut self, uri: &Uri) {
        self.globals_store.remove_workspace_folder(uri);
    }

    pub fn open_document(&mut self, uri: Uri, source: String) -> anyhow::Result<()> {
        tracing::info!("Opened {uri}");

        self.globals_store.seed(&uri)?;

        self.locals_store.load(uri, source)?;

        Ok(())
    }

    pub fn change_document(&mut self, uri: Uri, changes: Vec<Change>) -> anyhow::Result<()> {
        tracing::info!("Changed {uri}");

        self.locals_store.update(uri, changes)?;

        Ok(())
    }

    pub fn save_document(&mut self, uri: &Uri) -> anyhow::Result<()> {
        tracing::info!("Saved {uri}");

        let document = self.locals_store.document(uri)?;
        let source = document.to_bytes();

        self.globals_store.refresh(uri, source)?;

        Ok(())
    }

    pub fn close_document(&mut self, uri: Uri) -> anyhow::Result<()> {
        tracing::info!("Closed {uri}");

        self.globals_store.unseed(&uri);

        self.locals_store.unload(uri)?;

        Ok(())
    }

    pub fn find_definitions(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Vec<(Uri, PositionRange)>> {
        tracing::info!("Go to definition {uri} {position}");

        let document = self.locals_store.document(uri)?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        let definitions = document.find_definitions(&ident);
        let locations = definitions
            .into_iter()
            .map(|range| (uri.clone(), range))
            .collect_vec();
        if !locations.is_empty() {
            return Ok(locations);
        }

        let global_definitions = self.globals_store.find_definitions(&ident);
        let global_locations = global_definitions
            .into_iter()
            .filter(|s| &s.uri != uri)
            .map(|s| (s.uri, s.range))
            .collect_vec();

        Ok(global_locations)
    }

    pub fn find_references(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Vec<(Uri, PositionRange)>> {
        tracing::info!("Find references {uri} {position}");

        let document = self.locals_store.document(uri)?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        let (references, is_local) = document.find_references(&ident);
        let mut locations = references
            .into_iter()
            .map(|range| (uri.clone(), range))
            .collect_vec();

        if is_local {
            return Ok(locations);
        }

        let global_references = self.globals_store.find_references(&ident);
        locations.extend(
            global_references
                .into_iter()
                .filter(|s| &s.uri != uri)
                .map(|s| (s.uri, s.range)),
        );

        Ok(locations)
    }

    pub fn hover(&self, uri: &Uri, position: Position) -> anyhow::Result<Hover> {
        tracing::info!("Hover {uri} {position}");

        let definitions = self.find_definitions(uri, position)?;

        let mut cache = GlobalsHoverCache::default();
        let mut contents = definitions
            .into_iter()
            .map(|(uri, range)| {
                if let Some(document) = self.locals_store.document(&uri).ok() {
                    let snippet = document.hover(range);
                    MarkedString::LanguageString(LanguageString {
                        language: "c".to_owned(),
                        value: snippet,
                    })
                } else {
                    match cache.hover(uri, range, self.encoding) {
                        Ok(snippet) => MarkedString::LanguageString(LanguageString {
                            language: "c".to_owned(),
                            value: snippet,
                        }),
                        Err(err) => MarkedString::String(err.to_string()),
                    }
                }
            })
            .collect_vec();

        // Must come after all other document uses to avoid a deadlock.
        let document = self.locals_store.document(uri)?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        if contents.is_empty() {
            contents.push(MarkedString::String(format!(
                "No definition found for `{}`",
                String::from_utf8_lossy(&ident.bytes),
            )));
        }

        Ok(Hover {
            contents: HoverContents::Array(contents),
            range: Some(ident.range.into()),
        })
    }

    pub fn complete<'a>(
        &self,
        uri: &'a Uri,
        position: Position,
    ) -> anyhow::Result<impl Iterator<Item = (String, SymbolKind)> + 'a> {
        tracing::info!("Complete {uri} {position}");

        let document = self.locals_store.document(uri)?;

        let local_completions = document.completions(position);

        let global_definitions = self.globals_store.all_definitions();
        let global_completions = global_definitions
            .into_iter()
            .filter(move |s| &s.uri != uri)
            .map(|symbol| {
                (
                    String::from_utf8_lossy(&symbol.name).into_owned(),
                    symbol.kind,
                )
            });

        let keyword_completions = KEYWORDS
            .iter()
            .map(|&keyword| (keyword.to_owned(), SymbolKind::Keyword));

        let completions = local_completions
            .into_iter()
            .chain(global_completions)
            .chain(keyword_completions);

        Ok(completions)
    }
}
