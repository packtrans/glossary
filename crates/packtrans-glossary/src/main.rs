use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use packtrans_glossary_core::{QueryOptions, query_index};

#[derive(Parser)]
#[command(name = "packtrans-glossary")]
#[command(about = "Query Minecraft mod glossary translations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Query(QueryCommand),
}

#[derive(Args)]
struct QueryCommand {
    query: String,
    #[arg(long)]
    index_dir: PathBuf,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    inverse: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Query(command) => query_index(QueryOptions {
            query: command.query,
            index_db: command.index_dir,
            limit: command.limit,
            inverse: command.inverse,
        }),
    }
}
