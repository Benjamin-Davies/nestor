use anyhow::Context;
use itertools::Itertools;
use url::Url;

use crate::{
    analyze::{KEYWORDS, types::SymbolKind},
    globals::{GlobalsHoverCache, GlobalsStore},
    locals::LocalsStore,
    text::{Change, Position, PositionEncoding, PositionRange, markdown::Markdown},
};

pub struct Workspace {
    encoding: PositionEncoding,
    locals_store: LocalsStore,
    globals_store: GlobalsStore,
}

impl Workspace {
    pub fn new<'a>(encoding: PositionEncoding, folders: impl IntoIterator<Item = Url>) -> Self {
        let locals_store = LocalsStore::new(encoding);

        let mut globals_store = GlobalsStore::new(encoding);
        globals_store.set_workspace_folders(folders);

        Self {
            encoding,
            locals_store,
            globals_store,
        }
    }

    pub fn add_folder(&mut self, uri: &Url) {
        self.globals_store.add_workspace_folder(uri);
    }

    pub fn remove_folder(&mut self, uri: &Url) {
        self.globals_store.remove_workspace_folder(uri);
    }

    pub fn open_document(&mut self, uri: Url, source: String) -> anyhow::Result<()> {
        tracing::info!("Opened {uri}");

        self.globals_store.seed(&uri)?;

        self.locals_store.load(uri, source)?;

        Ok(())
    }

    pub fn change_document(&mut self, uri: Url, changes: Vec<Change>) -> anyhow::Result<()> {
        tracing::info!("Changed {uri}");

        self.locals_store.update(uri, changes)?;

        Ok(())
    }

    pub fn save_document(&mut self, uri: &Url) -> anyhow::Result<()> {
        tracing::info!("Saved {uri}");

        let document = self.locals_store.document(uri)?;
        let source = document.to_bytes();

        self.globals_store.refresh(uri, source)?;

        Ok(())
    }

    pub fn close_document(&mut self, uri: Url) -> anyhow::Result<()> {
        tracing::info!("Closed {uri}");

        self.globals_store.unseed(&uri);

        self.locals_store.unload(uri)?;

        Ok(())
    }

    pub fn find_definitions(
        &self,
        uri: &Url,
        position: Position,
    ) -> anyhow::Result<Vec<(Url, PositionRange)>> {
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
        uri: &Url,
        position: Position,
    ) -> anyhow::Result<Vec<(Url, PositionRange)>> {
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

    pub fn hover(
        &self,
        uri: &Url,
        position: Position,
    ) -> anyhow::Result<(Markdown, PositionRange)> {
        tracing::info!("Hover {uri} {position}");

        let definitions = self.find_definitions(uri, position)?;

        let mut cache = GlobalsHoverCache::default();
        let mut md = Markdown::default();
        for (uri, range) in definitions {
            if !md.is_empty() {
                md.push_rule();
            }

            if let Some(document) = self.locals_store.document(&uri).ok() {
                let hover = document.hover(range);
                md.append(hover);
            } else {
                match cache.hover(uri, range, self.encoding) {
                    Ok(hover) => md.append(hover),
                    Err(err) => {
                        md.build_paragraph().push_text(err.to_string());
                    }
                }
            }
        }

        // Must come after all other document uses to avoid a deadlock.
        let document = self.locals_store.document(uri)?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        if md.is_empty() {
            md.build_paragraph()
                .push_text("No definition found for ")
                .push_code(String::from_utf8_lossy(&ident.bytes).into_owned());
        }

        Ok((md, ident.range))
    }

    pub fn complete<'a>(
        &self,
        uri: &'a Url,
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
