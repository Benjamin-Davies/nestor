use std::path::PathBuf;

use clap::Parser as _;
use nestor::analyze::dirs;

#[derive(Debug, clap::Parser)]
struct Cli {
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = dirs::analyze(&cli.path)?;
    dbg!(result);

    Ok(())
}
