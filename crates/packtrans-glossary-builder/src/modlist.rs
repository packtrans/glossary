use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{CreateModListCommand, Platform};
use crate::util::{http_client, progress_bar};

pub fn create_mod_list(cmd: CreateModListCommand) -> Result<()> {
    match cmd.platform {
        Platform::Modrinth => fetch_modrinth_mod_list(&cmd.output, cmd.count),
        Platform::Curseforge => fetch_curseforge_mod_list(&cmd.output, cmd.count),
        Platform::Minecraft => {
            anyhow::bail!("create-mod-list does not support the minecraft platform")
        }
    }
}

fn fetch_modrinth_mod_list(output: &PathBuf, count: usize) -> Result<()> {
    let mut mods = Vec::new();
    let client = http_client();

    let pb = progress_bar(count as u64, "fetching mods");

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

            if let (Some(id), Some(slug), Some(name), Some(version_id)) =
                (id, slug, name, version_id)
            {
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
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
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

    let api_key = std::env::var("CURSEFORGE_API_KEY")
        .map_err(|_| anyhow::anyhow!("CURSEFORGE_API_KEY environment variable not set"))?;

    let mut mods = Vec::new();
    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let pb = progress_bar(count as u64, "fetching mods");

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

            let id = item
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string());
            let slug = item.get("slug").and_then(|v| v.as_str());
            let name = item.get("name").and_then(|v| v.as_str());

            let version_id = item
                .get("mainFileId")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string());

            if let (Some(id), Some(slug), Some(name), Some(version_id)) =
                (id, slug, name, version_id)
            {
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
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_string_pretty(&serde_json::Value::Array(mods))?,
    )?;
    println!("Wrote {} mods to {}", count, output.display());

    Ok(())
}
