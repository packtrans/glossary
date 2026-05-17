use anyhow::Result;
use clap::Parser;

use packtrans_glossary_core::{IndexOptions, build_index};

mod cli;
mod download;
mod modlist;
mod util;

use cli::{Cli, Commands};

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
