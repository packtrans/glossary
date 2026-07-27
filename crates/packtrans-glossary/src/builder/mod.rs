mod cli;
mod download;
mod index;
mod modlist;
mod util;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

pub use cli::{CreateModListCommand, DownloadCommand, IndexCommand, Platform};

use index::{IndexOptions, build_index};

#[derive(Args)]
pub struct BuilderArgs {
    #[command(subcommand)]
    command: BuilderCommands,
}

#[derive(Subcommand)]
enum BuilderCommands {
    /// Build a Tantivy search index from language files.
    Index(IndexCommand),
    /// Fetch a mod list from Modrinth, CurseForge, or Minecraft.
    CreateModList(CreateModListCommand),
    /// Download mod language files for indexing.
    Download(DownloadCommand),
}

pub fn run(args: BuilderArgs, dict_path: Option<PathBuf>) -> Result<()> {
    match args.command {
        BuilderCommands::Index(cmd) => build_index(IndexOptions {
            scan_dir: cmd.scan_dir,
            lang: cmd.lang,
            out: cmd.out,
            dict_path,
        }),
        BuilderCommands::CreateModList(cmd) => modlist::create_mod_list(cmd),
        BuilderCommands::Download(cmd) => download::download(cmd),
    }
}
