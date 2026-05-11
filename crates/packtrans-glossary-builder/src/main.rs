use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use packtrans_glossary_core::{IndexOptions, build_index};

#[derive(Parser)]
#[command(name = "packtrans_glossary_builder")]
#[command(about = "Build Minecraft mod glossary translation indexes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Index(IndexCommand),
}

#[derive(Args)]
struct IndexCommand {
    #[arg(long)]
    scan_dir: PathBuf,
    #[arg(long)]
    source: String,
    #[arg(long)]
    target: String,
    #[arg(long)]
    index_db: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index(command) => build_index(IndexOptions {
            scan_dir: command.scan_dir,
            source: command.source,
            target: command.target,
            index_db: command.index_db,
        }),
    }
}
