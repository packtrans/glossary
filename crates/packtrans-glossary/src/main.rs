use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};
use packtrans_glossary_core::dictionary;
use packtrans_glossary_core::{QueryOptions, query_index};

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
}

#[derive(Subcommand)]
enum Commands {
    Query(QueryCommand),
    Dict {
        #[command(subcommand)]
        command: DictCommand,
    },
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

#[derive(Subcommand)]
enum DictCommand {
    Download(DictDownloadCommand),
    Ls,
    Delete(DictDeleteCommand),
    Clean,
}

#[derive(Args)]
struct DictDownloadCommand {
    name: Option<String>,
}

#[derive(Args)]
struct DictDeleteCommand {
    name: String,
    #[arg(long)]
    version: Option<String>,
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
        }),
        Commands::Dict { command } => {
            let base = cli.dict_path.as_deref();
            match command {
                DictCommand::Download(cmd) => {
                    let names = match &cmd.name {
                        Some(name) => {
                            if !dictionary::DICTIONARY_NAMES.contains(&name.as_str()) {
                                bail!(
                                    "unknown dictionary '{}'. Available: {}",
                                    name,
                                    dictionary::DICTIONARY_NAMES.join(", ")
                                );
                            }
                            vec![name.clone()]
                        }
                        None => dictionary::DICTIONARY_NAMES
                            .iter()
                            .copied()
                            .map(String::from)
                            .collect(),
                    };
                    for name in &names {
                        let path = dictionary::ensure_dictionary(name, base)?;
                        println!("{} -> {}", name, path.display());
                    }
                    Ok(())
                }
                DictCommand::Ls => {
                    let entries = dictionary::list_dictionaries(base)?;
                    if entries.is_empty() {
                        println!("no dictionaries installed");
                        return Ok(());
                    }
                    println!("{:<20} {:<10} PATH", "NAME", "VERSION");
                    for entry in entries {
                        println!(
                            "{:<20} {:<10} {}",
                            entry.name,
                            entry.version,
                            entry.path.display()
                        );
                    }
                    Ok(())
                }
                DictCommand::Delete(cmd) => {
                    let version = cmd
                        .version
                        .as_deref()
                        .unwrap_or(dictionary::current_version());
                    dictionary::delete_dictionary(&cmd.name, version, base)?;
                    println!("deleted {}@{}", cmd.name, version);
                    Ok(())
                }
                DictCommand::Clean => {
                    let removed = dictionary::clean_old_versions(base)?;
                    if removed.is_empty() {
                        println!("no old versions to clean");
                    } else {
                        for version in &removed {
                            println!("removed version {}", version);
                        }
                    }
                    Ok(())
                }
            }
        }
    }
}
