use std::{fs, path::PathBuf};

use clap::Parser as _;
use nestor::analyze::{global::analyze, parse};

#[derive(Debug, clap::Parser)]
struct Cli {
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let source = fs::read(cli.path)?;
    let tree = parse(&source)?;

    let globals = analyze(tree.root_node(), &source);
    dbg!(&globals.symbols[..10]);
    dbg!(&globals.definitions[..10]);

    Ok(())
}
