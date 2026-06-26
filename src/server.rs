use std::ops::ControlFlow;

use crate::messages::{Notification, Request, Response};
use clap::{crate_name, crate_version};
use lsp_server::Message;
use lsp_types::{
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
    PositionEncodingKind, ReferenceParams,
};

struct Server {
    connection: lsp_server::Connection,
}

pub fn run_server(connection: lsp_server::Connection) -> anyhow::Result<()> {
    let (initialize_id, initialize_params) = connection.initialize_start()?;

    tracing::info!("InitializeParams: {}", initialize_params);
    let lsp_types::InitializeParams { .. } = serde_json::from_value(initialize_params)?;

    let initialize_result = serde_json::to_value(lsp_types::InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(lsp_types::ServerInfo {
            name: crate_name!().to_owned(),
            version: Some(crate_version!().to_owned()),
        }),
    })?;
    connection.initialize_finish(initialize_id, initialize_result)?;

    let server = Server { connection };
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
    fn handle_request(&self, request: lsp_server::Request) -> anyhow::Result<ControlFlow<()>> {
        match Request::try_from(request)? {
            Request::Shutdown(id) => {
                self.connection.sender.send(Response::Ok(id).into())?;
            }
            Request::GotoDefinition(
                id,
                GotoDefinitionParams {
                    text_document_position_params,
                    ..
                },
            ) => {
                tracing::info!("Go to definition {text_document_position_params:?}");
                self.connection.sender.send(Response::Ok(id).into())?;
            }
            Request::FindReferences(
                id,
                ReferenceParams {
                    text_document_position,
                    ..
                },
            ) => {
                tracing::info!("Find references {text_document_position:?}");
                self.connection.sender.send(Response::Ok(id).into())?;
            }
        }

        Ok(ControlFlow::Continue(()))
    }

    fn handle_notification(
        &self,
        notification: lsp_server::Notification,
    ) -> anyhow::Result<ControlFlow<()>> {
        match Notification::try_from(notification)? {
            Notification::Exit => return Ok(ControlFlow::Break(())),
            Notification::DidOpenTextDocument(DidOpenTextDocumentParams { text_document }) => {
                tracing::info!("Opened {}", text_document.uri.as_str());
            }
            Notification::DidCloseTextDocument(DidCloseTextDocumentParams { text_document }) => {
                tracing::info!("Closed {}", text_document.uri.as_str());
            }
            _ => {}
        }

        Ok(ControlFlow::Continue(()))
    }
}
