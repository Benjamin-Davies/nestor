use std::ops::ControlFlow;

use clap::{crate_name, crate_version};
use itertools::Itertools;
use lsp_server::Message;
use lsp_types::{
    CompletionItem, CompletionItemLabelDetails, CompletionList, CompletionParams, Contents,
    DefinitionParams, DidChangeTextDocumentParams, DidChangeWorkspaceFoldersParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover,
    HoverParams, Location, MarkupContent, MarkupKind, PositionEncodingKind, ReferenceParams,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier,
    WorkspaceFoldersChangeEvent,
};

use crate::{
    messages::{Notification, Request, Response},
    text::{Change, Position, PositionEncoding},
    workspace::Workspace,
};

struct Server {
    connection: lsp_server::Connection,
    workspace: Workspace,
}

pub fn run_server(connection: lsp_server::Connection) -> anyhow::Result<()> {
    let (initialize_id, initialize_params) = connection.initialize_start()?;

    let init_params = serde_json::from_value::<lsp_types::InitializeParams>(initialize_params)?;

    let encoding = choose_position_encoding(&init_params.capabilities);

    let initialize_result = serde_json::to_value(lsp_types::InitializeResult {
        capabilities: server_capabilities(encoding),
        server_info: Some(lsp_types::ServerInfo {
            name: crate_name!().to_owned(),
            version: Some(crate_version!().to_owned()),
        }),
    })?;
    connection.initialize_finish(initialize_id, initialize_result)?;

    let folders = init_params
        .workspace_folders_initialize_params
        .workspace_folders
        .and_then(|f| match f {
            lsp_types::WorkspaceFolders::WorkspaceFolderList(folders) => Some(folders),
            lsp_types::WorkspaceFolders::Null => None,
        })
        .into_iter()
        .flatten()
        .map(|f| f.uri.into());
    let mut server = Server {
        connection,
        workspace: Workspace::new(encoding, folders),
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

fn choose_position_encoding(capabilities: &lsp_types::ClientCapabilities) -> PositionEncoding {
    let mut has_utf8 = false;
    if let Some(general) = &capabilities.general {
        if let Some(encodings) = &general.position_encodings {
            has_utf8 = encodings.contains(&PositionEncodingKind::UTF8);
        }
    }

    if has_utf8 {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    }
}

fn server_capabilities(encoding: PositionEncoding) -> lsp_types::ServerCapabilities {
    lsp_types::ServerCapabilities {
        position_encoding: Some(encoding.into()),
        text_document_sync: Some(lsp_types::TextDocumentSync::Options(
            lsp_types::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lsp_types::TextDocumentSyncKind::Incremental),
                will_save: None,
                will_save_wait_until: None,
                save: Some(lsp_types::Save::Bool(true)),
            },
        )),
        definition_provider: Some(lsp_types::DefinitionProvider::Bool(true)),
        references_provider: Some(lsp_types::ReferencesProvider::Bool(true)),
        hover_provider: Some(lsp_types::HoverProvider::Bool(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            completion_item: Some(lsp_types::ServerCompletionItemOptions {
                label_details_support: Some(true),
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Server {
    fn handle_request(&mut self, request: lsp_server::Request) -> anyhow::Result<ControlFlow<()>> {
        match Request::try_from(request)? {
            Request::Shutdown(id) => {
                self.connection.sender.send(Response::Ok(id).into())?;
            }
            Request::Definition(
                id,
                DefinitionParams {
                    text_document_position_params:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                let locations = self
                    .workspace
                    .find_definitions(&uri, position.into())?
                    .into_iter()
                    .map(|(uri, range)| Location {
                        uri,
                        range: range.into(),
                    })
                    .collect_vec();

                self.connection
                    .sender
                    .send(Response::Locations(id, locations).into())?;
            }
            Request::FindReferences(
                id,
                ReferenceParams {
                    text_document_position_params:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                let locations = self
                    .workspace
                    .find_references(&uri.into(), position.into())?
                    .into_iter()
                    .map(|(uri, range)| {
                        Ok(Location {
                            uri: uri.try_into()?,
                            range: range.into(),
                        })
                    })
                    .collect::<anyhow::Result<_>>()?;

                self.connection
                    .sender
                    .send(Response::Locations(id, locations).into())?;
            }
            Request::Hover(
                id,
                HoverParams {
                    text_document_position_params:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                let hover = self
                    .workspace
                    .hover(&uri.into(), position.into())
                    .inspect_err(|err| tracing::warn!("Error resolving hover: {err}"))
                    .ok()
                    .map(|(contents, range)| Hover {
                        contents: Contents::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: contents.to_string(),
                        }),
                        range: Some(range.into()),
                    });

                self.connection
                    .sender
                    .send(Response::Hover(id, hover).into())?;
            }
            Request::Completion(
                id,
                CompletionParams {
                    text_document_position_params:
                        TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri },
                            position,
                        },
                    ..
                },
            ) => {
                let list = self.complete(uri, position.into())?;

                self.connection
                    .sender
                    .send(Response::CompletionList(id, list).into())?;
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
                self.workspace
                    .open_document(text_document.uri.into(), text_document.text)?;
            }
            Notification::DidChangeTextDocument(DidChangeTextDocumentParams {
                text_document:
                    VersionedTextDocumentIdentifier {
                        text_document_identifier: TextDocumentIdentifier { uri },
                        ..
                    },
                content_changes,
            }) => {
                let changes = content_changes.into_iter().map(Change::from).collect_vec();
                self.workspace.change_document(uri.into(), changes)?;
            }
            Notification::DidSaveTextDocument(DidSaveTextDocumentParams {
                text_document, ..
            }) => {
                self.workspace.save_document(&text_document.uri.into())?;
            }
            Notification::DidCloseTextDocument(DidCloseTextDocumentParams { text_document }) => {
                self.workspace.close_document(text_document.uri.into())?;
            }
            Notification::DidChangeWorkspaceFolders(DidChangeWorkspaceFoldersParams {
                event: WorkspaceFoldersChangeEvent { added, removed },
            }) => {
                for folder in added {
                    self.workspace.add_folder(&folder.uri.into());
                }
                for folder in removed {
                    self.workspace.remove_folder(&folder.uri.into());
                }
            }
            _ => {}
        }

        Ok(ControlFlow::Continue(()))
    }

    pub fn complete(&self, uri: Uri, position: Position) -> anyhow::Result<CompletionList> {
        let items = self
            .workspace
            .complete(&uri.into(), position.into())?
            .map(|(name, kind)| CompletionItem {
                label: name,
                label_details: Some(CompletionItemLabelDetails {
                    detail: Some(kind.to_string()),
                    description: None,
                }),
                detail: Some(kind.to_string()),
                kind: kind.into(),
                ..Default::default()
            })
            .collect_vec();

        Ok(CompletionList {
            is_incomplete: false,
            items,
            ..Default::default()
        })
    }
}
