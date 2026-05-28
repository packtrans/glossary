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
            index_dir: cmd.index_dir,
            lang: cmd.lang,
            limit: cmd.limit,
            inverse: cmd.inverse,
            dict_path: cli.dict_path,
        }),
        Commands::Dict { command } => dict::run(command, cli.dict_path.as_deref()),
        Commands::Index { command } => indexes::run(command),
    }
}

#[derive(Args)]
struct QueryCommand {
    query: String,

    /// Local index root directory; queries `{index_dir}/{lang}` (same layout as `index --out`).
    #[arg(long)]
    index_dir: Option<PathBuf>,

    #[arg(long)]
    lang: String,

    #[arg(long, default_value_t = 10)]
    limit: usize,

    #[arg(long)]
    inverse: bool,
}
