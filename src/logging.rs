use std::{
    fs::File,
    io::{self, Write},
    sync::OnceLock,
};

use crossbeam_channel::Sender;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    prelude::*,
};

static LSP_SENDER: OnceLock<Sender<lsp_server::Message>> = OnceLock::new();

struct LogWriter {
    file: File,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let message = String::from_utf8_lossy(buf).into_owned();

        if let Some(sender) = LSP_SENDER.get() {
            let _ = sender.send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "window/logMessage".to_owned(),
                    // TODO: strongly typed
                    params: serde_json::json!({
                        // MessageType::Log == 4
                        "type": 4,
                        "message": message,
                    }),
                },
            ));
        }

        let ret = self.file.write(buf)?;

        Ok(ret)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct MakeLogWriter;

impl<'a> MakeWriter<'a> for MakeLogWriter {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        let file = File::options()
            .create(true)
            .append(true)
            .open("lsp.log") // TODO: choose a better location
            .expect("failed to create log file");

        LogWriter { file }
    }
}

pub fn init(max_level: tracing::Level) {
    let fmt_layer = fmt::layer()
        .with_writer(MakeLogWriter)
        .with_ansi(false)
        .with_level(true)
        .with_target(true);

    tracing_subscriber::registry()
        .with(fmt_layer.with_filter(LevelFilter::from_level(max_level)))
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
