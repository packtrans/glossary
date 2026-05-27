use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod dict;
mod indexes;
mod progress;
mod query;

use dict::DictCommand;
use indexes::IndexCommand;
use query::{QueryOptions, query_index};

#[derive(Parser)]
#[command(name = "packtrans-glossary")]
#[command(about = "Query Minecraft mod glossary translations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long)]
    dict_path: Option<PathBuf>,

    #[arg(long)]
    index_path: Option<PathBuf>,

    /// Use a locally built index under `local/{lang}` instead of a release download.
    #[arg(long)]
    prefer_local_index: bool,
}

#[derive(Subcommand)]
enum Commands {
    Query(QueryCommand),
    Dict {
        #[command(subcommand)]
        command: DictCommand,
    },
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Query(cmd) => query_index(QueryOptions {
            query: cmd.query,
            index_path: cli.index_path,
            lang: cmd.lang,
            limit: cmd.limit,
            inverse: cmd.inverse,
            dict_path: cli.dict_path,
            prefer_local_index: cli.prefer_local_index,
        }),
        Commands::Dict { command } => dict::run(command, cli.dict_path.as_deref()),
        Commands::Index { command } => indexes::run(command, cli.index_path.as_deref()),
    }
}

#[derive(Args)]
struct QueryCommand {
    query: String,

    #[arg(long)]
    lang: String,

    #[arg(long, default_value_t = 20)]
    limit: usize,

    #[arg(long)]
    inverse: bool,
}
