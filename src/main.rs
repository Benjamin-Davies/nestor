use std::process::ExitCode;

use nestor::{logging, server::run_server};

fn main() -> ExitCode {
    logging::init(tracing::Level::INFO);

    tracing::info!("Starting lsp server");
    let (connection, io_threads) = lsp_server::Connection::stdio();
    logging::set_lsp_sender(connection.sender.clone());

    let ret = match run_server(connection) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Print error before we close the connection.
            tracing::error!("Error: {err}");

            ExitCode::FAILURE
        }
    };

    io_threads.join().expect("Error while joining IO threads");

    ret
}
