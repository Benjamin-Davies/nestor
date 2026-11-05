use std::{fmt, sync::OnceLock};

use crossbeam_channel::Sender;
use lsp_types::{LogMessageParams, MessageType};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    level_filters::LevelFilter,
};
use tracing_subscriber::{
    fmt::{
        format::Writer,
        time::{FormatTime, SystemTime},
    },
    layer::Layer,
    prelude::*,
};

use crate::messages::Notification;

static LSP_SENDER: OnceLock<Sender<lsp_server::Message>> = OnceLock::new();

struct LspLayer;

impl<S: Subscriber> Layer<S> for LspLayer {
    fn on_event(&self, event: &tracing::Event, _ctx: tracing_subscriber::layer::Context<S>) {
        let level = *event.metadata().level();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_default();

        let message = format!("[{Now} {level}] {message}");

        if let Some(sender) = LSP_SENDER.get() {
            let _ = sender.send(
                Notification::LogMessage(LogMessageParams {
                    typ: level_to_message_type(level),
                    message,
                })
                .into(),
            );
        } else {
            eprintln!("{message}");
        }
    }
}

fn level_to_message_type(level: tracing::Level) -> MessageType {
    if level <= tracing::Level::ERROR {
        MessageType::ERROR
    } else if level <= tracing::Level::WARN {
        MessageType::WARNING
    } else if level <= tracing::Level::INFO {
        MessageType::INFO
    } else {
        MessageType::LOG
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }
}

struct Now;

impl fmt::Display for Now {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        SystemTime.format_time(&mut Writer::new(f))
    }
}

pub fn init(max_level: tracing::Level) {
    tracing_subscriber::registry()
        .with(LspLayer.with_filter(LevelFilter::from_level(max_level)))
        .init();
}

pub fn set_lsp_sender(sender: Sender<lsp_server::Message>) {
    LSP_SENDER
        .set(sender)
        .expect("Can only set the logging LSP sender once");
}

#[cfg(test)]
mod tests {
    use lsp_server::Message;

    use crate::logging::{init, set_lsp_sender};

    #[test]
    fn single_event_produces_one_notification() {
        init(tracing::Level::DEBUG);

        let (tx, rx) = crossbeam_channel::unbounded();
        set_lsp_sender(tx);

        tracing::info!("hello from the LSP server");

        let msg = rx.try_recv().expect("expected a message");
        match msg {
            Message::Notification(n) => {
                assert_eq!(n.method, "window/logMessage");
                let text = n.params["message"].as_str().unwrap();
                assert!(text.contains("hello from the LSP server"), "got: {text}");
            }
            other => panic!("unexpected message kind: {other:?}"),
        }
    }
}
