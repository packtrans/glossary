use std::path::PathBuf;

use clap::{Args, ValueEnum};

#[derive(Args)]
pub struct IndexCommand {
    /// Directory to scan for language JSON files.
    #[arg(long)]
    pub scan_dir: PathBuf,
    /// Target language code (e.g. `zh_cn`, `ja_jp`).
    #[arg(long)]
    pub lang: String,
    /// Index root directory; the index is written to `{out}/{lang}`.
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct CreateModListCommand {
    /// Platform to fetch mods from.
    #[arg(short, long, value_enum)]
    pub platform: Platform,
    /// Output path for the mod list JSON file.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Maximum number of mods to include.
    #[arg(short, long, default_value = "1000")]
    pub count: usize,
}

#[derive(Args)]
pub struct DownloadCommand {
    /// Platform to download language files from.
    #[arg(short, long, value_enum)]
    pub platform: Platform,
    /// Output directory for downloaded language files.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Temporary directory for downloads.
    #[arg(long)]
    pub temp_path: Option<PathBuf>,
    /// Mod list JSON file (required for Modrinth and CurseForge).
    #[arg(short = 'f', long)]
    pub list_file: Option<PathBuf>,
}

#[derive(Clone, ValueEnum)]
pub enum Platform {
    /// Modrinth mod platform.
    Modrinth,
    /// CurseForge mod platform (requires `CURSEFORGE_API_KEY`).
    Curseforge,
    /// Minecraft vanilla language files.
    Minecraft,
}
