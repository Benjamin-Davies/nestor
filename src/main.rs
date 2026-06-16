use clap::{crate_name, crate_version};
use lsp_types::PositionEncodingKind;
use nestor::logging;

fn main() -> anyhow::Result<()> {
    logging::init(tracing::Level::INFO);

    tracing::info!("Starting lsp server");
    let (connection, io_threads) = lsp_server::Connection::stdio();
    logging::set_lsp_sender(connection.sender.clone());

    let (initialize_id, initialize_params) = match connection.initialize_start() {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };

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

    loop {
        let message = connection.receiver.recv()?;
        match message {
            lsp_server::Message::Request(request) => match request.method.as_str() {
                "shutdown" => {
                    connection.sender.send(lsp_server::Message::Response(
                        lsp_server::Response {
                            id: request.id,
                            result: None,
                            error: None,
                        },
                    ))?;
                }
                "exit" => break,
                "textDocument/definition" => {
                    let lsp_types::GotoDefinitionParams {
                        text_document_position_params,
                        ..
                    } = serde_json::from_value(request.params)?;
                    tracing::info!("Go to definition {text_document_position_params:?}")
                }
                m => tracing::warn!("Unknown request method {m:?}"),
            },
            lsp_server::Message::Response(response) => unimplemented!("{response:?}"),
            lsp_server::Message::Notification(notification) => match notification.method.as_str() {
                "textDocument/didOpen" => {
                    let lsp_types::DidOpenTextDocumentParams { text_document } =
                        serde_json::from_value(notification.params)?;
                    tracing::info!("Opened {}", text_document.uri.as_str());
                }
                m => tracing::warn!("Unknown notification method {m:?}"),
            },
        }
    }

    io_threads.join()?;

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
