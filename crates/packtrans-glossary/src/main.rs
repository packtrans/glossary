use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

mod app_state;
mod dict;
mod dict_cache;
mod download_guard;
mod index_cache;
mod indexes;
mod mcp;
mod progress;
mod query;
mod serve;

use dict::DictCommand;
use indexes::IndexCommand;
use mcp::McpCommand;
use query::{QueryOptions, query_index};
use serve::ServeCommand;

#[derive(Parser)]
#[command(name = "packtrans-glossary")]
#[command(about = "Query Minecraft mod glossary translations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Base directory for Lindera tokenizer dictionaries.
    #[arg(long)]
    dict_path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search glossary translations.
    Query(QueryCommand),
    /// Start an HTTP API server for queries (experimental, local use only).
    Serve(ServeCommand),
    /// Start an MCP server for glossary tools (stdio or HTTP).
    Mcp(McpCommand),
    /// Manage Lindera tokenizer dictionaries.
    Dict {
        #[command(subcommand)]
        command: DictCommand,
    },
    /// Manage release-downloaded search indexes.
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Query(cmd) => query_index(
            QueryOptions {
                query: cmd.query,
                index_dir: cmd.index_dir,
                lang: cmd.lang,
                limit: cmd.limit,
                inverse: cmd.inverse,
                regex: cmd.regex,
                dict_path: cli.dict_path,
                download_guard: None,
                dict_cache: None,
                index_cache: None,
            },
            cmd.json,
        ),
        Commands::Serve(cmd) => {
            let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
            rt.block_on(serve::run(cmd, cli.dict_path))
        }
        Commands::Mcp(cmd) => {
            let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
            rt.block_on(mcp::run(cmd, cli.dict_path))
        }
        Commands::Dict { command } => dict::run(command, cli.dict_path.as_deref()),
        Commands::Index { command } => indexes::run(command),
    }
}

#[derive(Args)]
struct QueryCommand {
    /// Search text.
    query: String,

    /// Local index root directory; queries `{index_dir}/{lang}` (same layout as `index --out`).
    #[arg(long)]
    index_dir: Option<PathBuf>,

    /// Target language code (e.g. `zh_cn`, `ja_jp`).
    #[arg(long)]
    lang: String,

    /// Maximum number of results to return.
    #[arg(long, default_value_t = 10)]
    limit: usize,

    /// Search target-language text and return source-language matches.
    #[arg(long)]
    inverse: bool,

    /// Interpret the query as a regular expression matching indexed terms.
    #[arg(long)]
    regex: bool,

    /// Print results as JSON (same shape as the `serve` HTTP API).
    #[arg(long)]
    json: bool,
}
