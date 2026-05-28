use std::path::Path;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use packtrans_glossary_core::dictionary;

use crate::progress;

#[derive(Subcommand)]
pub enum DictCommand {
    Download(DictDownloadCommand),
    Ls,
    Delete(DictDeleteCommand),
    Clean,
}

#[derive(Args)]
pub struct DictDownloadCommand {
    name: Option<String>,
}

#[derive(Args)]
pub struct DictDeleteCommand {
    name: String,
    #[arg(long)]
    version: Option<String>,
}

pub fn run(command: DictCommand, base: Option<&Path>) -> Result<()> {
    match command {
        DictCommand::Download(cmd) => download(cmd, base),
        DictCommand::Ls => list(base),
        DictCommand::Delete(cmd) => delete(cmd, base),
        DictCommand::Clean => clean(base),
    }
}

fn download(cmd: DictDownloadCommand, base: Option<&Path>) -> Result<()> {
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
        let expected = dictionary::dictionary_path(name, base)?;
        let path = if expected.is_dir() {
            expected
        } else {
            let pb = progress::spinner(format!("Downloading {name} dictionary"));
            let result = dictionary::ensure_dictionary(name, base);
            match result {
                Ok(path) => {
                    pb.finish_and_clear();
                    path
                }
                Err(err) => {
                    pb.finish_and_clear();
                    return Err(err);
                }
            }
        };
        println!("{} -> {}", name, path.display());
    }

    Ok(())
}

fn list(base: Option<&Path>) -> Result<()> {
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

fn delete(cmd: DictDeleteCommand, base: Option<&Path>) -> Result<()> {
    let version = cmd
        .version
        .as_deref()
        .unwrap_or(dictionary::current_version());
    dictionary::delete_dictionary(&cmd.name, version, base)?;
    println!("deleted {}@{}", cmd.name, version);
    Ok(())
}

fn clean(base: Option<&Path>) -> Result<()> {
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
