use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use packtrans_glossary_core::{IndexOptions, build_index};

#[derive(Parser)]
#[command(name = "packtrans-glossary-builder")]
#[command(about = "Build Minecraft mod glossary translation indexes")]
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
    Index(IndexCommand),
    CreateModList(CreateModListCommand),
}

#[derive(Args)]
struct IndexCommand {
    #[arg(long)]
    scan_dir: PathBuf,
    #[arg(long)]
    lang: String,
}

#[derive(Args)]
struct CreateModListCommand {
    #[arg(short, long, value_enum)]
    platform: Platform,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(short, long, default_value = "1000")]
    count: usize,
}

#[derive(Clone, ValueEnum)]
enum Platform {
    Modrinth,
    Curseforge,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index(cmd) => build_index(IndexOptions {
            scan_dir: cmd.scan_dir,
            lang: cmd.lang,
            index_path: cli.index_path,
            dict_path: cli.dict_path,
        }),
        Commands::CreateModList(cmd) => create_mod_list(cmd),
    }
}

fn create_mod_list(cmd: CreateModListCommand) -> Result<()> {
    match cmd.platform {
        Platform::Modrinth => fetch_modrinth_mod_list(&cmd.output, cmd.count),
        Platform::Curseforge => fetch_curseforge_mod_list(&cmd.output, cmd.count),
    }
}

fn fetch_modrinth_mod_list(output: &PathBuf, count: usize) -> Result<()> {
    let mut mods = Vec::new();
    let user_agent = format!(
        "packtrans/glossary/{} (https://github.com/packtrans/glossary)",
        env!("CARGO_PKG_VERSION")
    );
    let client = ureq::AgentBuilder::new()
        .user_agent(&user_agent)
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({msg})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("fetching mods");

    let mut offset = 0usize;
    while mods.len() < count {
        let mods_len_before = mods.len();
        let url = format!(
            "https://api.modrinth.com/v2/search?facets=%5B%5B%22project_type%3Amod%22%5D%5D&index=downloads&limit=100&offset={}",
            offset
        );

        let response = client.get(&url).call()?;
        let json: serde_json::Value = response.into_json()?;

        let hits = json
            .get("hits")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing hits array in response"))?;

        for hit in hits {
            if mods.len() >= count {
                break;
            }

            let id = hit.get("project_id").and_then(|v| v.as_str());
            let slug = hit.get("slug").and_then(|v| v.as_str());
            let name = hit.get("title").and_then(|v| v.as_str());
            let version_id = hit.get("latest_version").and_then(|v| v.as_str());

            if let (Some(id), Some(slug), Some(name), Some(version_id)) = (id, slug, name, version_id) {
                mods.push(serde_json::json!({
                    "id": id,
                    "slug": slug,
                    "name": name,
                    "version_id": version_id,
                }));
            }
        }

        let actual_added = mods.len() - mods_len_before;
        pb.inc(actual_added as u64);

        if hits.len() < 100 || mods.len() >= count {
            break;
        }
        offset += 100;
    }

    pb.finish_with_message("done");

    let count = mods.len();
    fs::write(
        output,
        serde_json::to_string_pretty(&serde_json::Value::Array(mods))?,
    )?;
    println!("Wrote {} mods to {}", count, output.display());

    Ok(())
}

fn fetch_curseforge_mod_list(output: &PathBuf, count: usize) -> Result<()> {
    const CURSEFORGE_PAGE_SIZE: usize = 50;
    const CURSEFORGE_MAX_COUNT: usize = 10_000;

    anyhow::ensure!(
        count > 0,
        "fetch_curseforge_mod_list: count must be > 0, got {}",
        count
    );
    anyhow::ensure!(
        count <= CURSEFORGE_MAX_COUNT,
        "fetch_curseforge_mod_list: count must be <= {} (CurseForge API: index + pageSize cannot exceed 10000), got {}",
        CURSEFORGE_MAX_COUNT,
        count
    );

    let api_key = env::var("CURSEFORGE_API_KEY")
        .map_err(|_| anyhow::anyhow!("CURSEFORGE_API_KEY environment variable not set"))?;

    let mut mods = Vec::new();
    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({msg})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("fetching mods");

    let mut offset = 0usize;
    while mods.len() < count {
        if offset + CURSEFORGE_PAGE_SIZE > CURSEFORGE_MAX_COUNT {
            eprintln!(
                "warning: CurseForge pagination cap reached (offset {offset} + page_size {CURSEFORGE_PAGE_SIZE} > {CURSEFORGE_MAX_COUNT}); stopping with {} mod(s) (requested {count}).",
                mods.len(),
            );
            break;
        }

        let mods_len_before = mods.len();
        let url = format!(
            "https://api.curseforge.com/v1/mods/search?gameId=432&sortField=6&sortOrder=desc&pageSize={}&index={}",
            CURSEFORGE_PAGE_SIZE, offset
        );

        let response = client.get(&url).set("x-api-key", &api_key).call()?;
        let json: serde_json::Value = response.into_json()?;

        let data = json
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing data array in CurseForge response"))?;

        for item in data {
            if mods.len() >= count {
                break;
            }

            let id = item.get("id").and_then(|v| v.as_i64()).map(|v| v.to_string());
            let slug = item.get("slug").and_then(|v| v.as_str());
            let name = item.get("name").and_then(|v| v.as_str());

            let version_id = item
                .get("mainFileId")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string());

            if let (Some(id), Some(slug), Some(name), Some(version_id)) = (id, slug, name, version_id) {
                mods.push(serde_json::json!({
                    "id": id,
                    "slug": slug,
                    "name": name,
                    "version_id": version_id,
                }));
            }
        }

        let actual_added = mods.len() - mods_len_before;
        pb.inc(actual_added as u64);

        if data.len() < CURSEFORGE_PAGE_SIZE || mods.len() >= count {
            break;
        }
        offset += CURSEFORGE_PAGE_SIZE;
    }

    pb.finish_with_message("done");

    let count = mods.len();
    fs::write(
        output,
        serde_json::to_string_pretty(&serde_json::Value::Array(mods))?,
    )?;
    println!("Wrote {} mods to {}", count, output.display());

    Ok(())
}
