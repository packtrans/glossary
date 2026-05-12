use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use packtrans_glossary_core::{IndexOptions, build_index};

#[derive(Parser)]
#[command(name = "packtrans-glossary-builder")]
#[command(about = "Build Minecraft mod glossary translation indexes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long)]
    dict_path: Option<PathBuf>,

    #[arg(long)]
    index_path: Option<PathBuf>,
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
    lang: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index(cmd) => build_index(IndexOptions {
            scan_dir: cmd.scan_dir,
            lang: cmd.lang,
            index_path: cli.index_path,
            dict_path: cli.dict_path,
        }),
    }
}
