use anyhow::Result;
use clap::Parser;

mod cli;
mod download;
mod index;
mod modlist;
mod util;

use cli::{Cli, Commands};
use index::{IndexOptions, build_index};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index(cmd) => build_index(IndexOptions {
            scan_dir: cmd.scan_dir,
            lang: cmd.lang,
            index_path: cli.index_path,
            dict_path: cli.dict_path,
        }),
        Commands::CreateModList(cmd) => modlist::create_mod_list(cmd),
        Commands::Download(cmd) => download::download(cmd),
    }
}
