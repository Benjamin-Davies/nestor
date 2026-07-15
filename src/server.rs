use std::{collections::BTreeMap, ops::ControlFlow};

use anyhow::Context;
use clap::{crate_name, crate_version};
use itertools::Itertools;
use lsp_server::Message;
use lsp_types::{
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, Location, Position, PositionEncodingKind, ReferenceParams,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
};

use crate::{
    document::Document,
    globals::GlobalsStore,
    messages::{Notification, Request, Response},
};

struct Server {
    connection: lsp_server::Connection,
    documents: BTreeMap<Uri, Document>,
    globals_store: GlobalsStore,
}

pub fn run_server(connection: lsp_server::Connection) -> anyhow::Result<()> {
    let (initialize_id, initialize_params) = connection.initialize_start()?;

    let lsp_types::InitializeParams {
        workspace_folders, ..
    } = serde_json::from_value(initialize_params)?;

    let initialize_result = serde_json::to_value(lsp_types::InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(lsp_types::ServerInfo {
            name: crate_name!().to_owned(),
            version: Some(crate_version!().to_owned()),
        }),
    })?;
    connection.initialize_finish(initialize_id, initialize_result)?;

    let mut globals_store = GlobalsStore::new();
    globals_store.set_workspace_folders(&workspace_folders.unwrap_or_default());

    let mut server = Server {
        connection: connection,
        documents: BTreeMap::new(),
        globals_store,
    };
    loop {
        let message = server.connection.receiver.recv()?;
        match message {
            Message::Request(request) => match server.handle_request(request) {
                Ok(ControlFlow::Continue(())) => {}
                Ok(ControlFlow::Break(())) => break,
                Err(err) => tracing::error!("Error handling request: {err}"),
            },
            Message::Response(response) => {
                tracing::warn!("Received unexpected response: {response:?}");
            }
            Message::Notification(notification) => match server.handle_notification(notification) {
                Ok(ControlFlow::Continue(())) => {}
                Ok(ControlFlow::Break(())) => break,
                Err(err) => tracing::error!("Error handling notification: {err}"),
            },
        }
    }

    Ok(())
}

fn server_capabilities() -> lsp_types::ServerCapabilities {
    lsp_types::ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF8),
        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::FULL,
        )),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        references_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    }
}

impl Server {
    fn handle_request(&mut self, request: lsp_server::Request) -> anyhow::Result<ControlFlow<()>> {
        match Request::try_from(request)? {
            Request::Shutdown(id) => {
                self.connection.sender.send(Response::Ok(id).into())?;
            }
            Request::GotoDefinition(
                id,
                GotoDefinitionParams {
                    text_document_position_params:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                tracing::info!("Go to definition {} {position:?}", uri.as_str());

                let locations = self.find_definitions(&uri, position)?;

                self.connection
                    .sender
                    .send(Response::Locations(id, locations).into())?;
            }
            Request::FindReferences(
                id,
                ReferenceParams {
                    text_document_position:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                tracing::info!("Find references {} {position:?}", uri.as_str());

                let locations = self.find_references(&uri, position)?;

                self.connection
                    .sender
                    .send(Response::Locations(id, locations).into())?;
            }
        }

        Ok(ControlFlow::Continue(()))
    }

    fn handle_notification(
        &mut self,
        notification: lsp_server::Notification,
    ) -> anyhow::Result<ControlFlow<()>> {
        match Notification::try_from(notification)? {
            Notification::Exit => return Ok(ControlFlow::Break(())),
            Notification::DidOpenTextDocument(DidOpenTextDocumentParams { text_document }) => {
                tracing::info!("Opened {}", text_document.uri.as_str());

                self.load_document(text_document)?;
            }
            Notification::DidCloseTextDocument(DidCloseTextDocumentParams { text_document }) => {
                tracing::info!("Closed {}", text_document.uri.as_str());

                self.discard_document(&text_document.uri);
            }
            Notification::DidChangeWorkspaceFolders(DidChangeWorkspaceFoldersParams { event }) => {
                for added in &event.added {
                    self.globals_store.add_workspace_folder(added);
                }
                for removed in &event.removed {
                    self.globals_store.remove_workspace_folder(removed);
                }
            }
            _ => {}
        }

        Ok(ControlFlow::Continue(()))
    }

    fn load_document(&mut self, text_document: TextDocumentItem) -> anyhow::Result<()> {
        self.globals_store.seed(&text_document.uri)?;

        let uri = text_document.uri.clone();
        let document = Document::try_from(text_document)?;
        self.documents.insert(uri, document);

        Ok(())
    }

    fn discard_document(&mut self, uri: &Uri) {
        self.documents.remove(uri);

        self.globals_store.unseed(uri);
    }

    fn find_definitions(
        &mut self,
        uri: &Uri,
        position: Position,
    ) -> Result<Vec<Location>, anyhow::Error> {
        let document = self.documents.get(uri).context("No such open document")?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        let definitions = document.find_definitions(ident);
        let locations = definitions
            .into_iter()
            .map(|range| Location {
                uri: uri.clone(),
                range: range.into(),
            })
            .collect_vec();
        if !locations.is_empty() {
            return Ok(locations);
        }

        let global_definitions = self
            .globals_store
            .find_definitions(document.bytes_for(ident));
        let global_locations = global_definitions
            .into_iter()
            .filter(|s| &s.uri != uri)
            .map(|s| Location {
                uri: s.uri,
                range: s.range.into(),
            })
            .collect_vec();

        Ok(global_locations)
    }

    fn find_references(&self, uri: &Uri, position: Position) -> anyhow::Result<Vec<Location>> {
        let document = self.documents.get(uri).context("No such open document")?;
        let ident = document
            .ident_at(position.into())
            .context("No ident at cursor")?;

        let (references, is_local) = document.find_references(ident);
        let mut locations = references
            .into_iter()
            .map(|range| Location {
                uri: uri.clone(),
                range: range.into(),
            })
            .collect_vec();

        if is_local {
            return Ok(locations);
        }

        let global_references = self
            .globals_store
            .find_references(document.bytes_for(ident));
        locations.extend(
            global_references
                .into_iter()
                .filter(|s| &s.uri != uri)
                .map(|s| Location {
                    uri: s.uri,
                    range: s.range.into(),
                }),
        );

        Ok(locations)
    }
}
