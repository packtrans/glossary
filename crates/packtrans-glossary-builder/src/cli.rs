use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "packtrans-glossary-builder")]
#[command(about = "Build Minecraft mod glossary translation indexes")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long)]
    pub dict_path: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Index(IndexCommand),
    CreateModList(CreateModListCommand),
    Download(DownloadCommand),
}

#[derive(Args)]
pub struct IndexCommand {
    #[arg(long)]
    pub scan_dir: PathBuf,
    #[arg(long)]
    pub lang: String,
    /// Index root directory; the index is written to `{out}/{lang}`.
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct CreateModListCommand {
    #[arg(short, long, value_enum)]
    pub platform: Platform,
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(short, long, default_value = "1000")]
    pub count: usize,
}

#[derive(Args)]
pub struct DownloadCommand {
    #[arg(short, long, value_enum)]
    pub platform: Platform,
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub temp_path: Option<PathBuf>,
    #[arg(short = 'f', long)]
    pub list_file: Option<PathBuf>,
}

#[derive(Clone, ValueEnum)]
pub enum Platform {
    Modrinth,
    Curseforge,
    Minecraft,
}
