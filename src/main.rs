use std::{fs, path::PathBuf};

use clap::Parser;
use nestor::scanner::Scanner;

#[derive(Debug, Parser)]
struct Cli {
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let input = fs::read_to_string(cli.path)?;
    for token in Scanner::new(&input) {
        println!("{token:?}");
    }

    Ok(())
}
