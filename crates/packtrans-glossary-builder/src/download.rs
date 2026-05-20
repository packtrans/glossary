use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use packtrans_glossary_core::util::{
    copy_dir_contents, download_to_file, extract_zip_file, find_best_lang_dir, sanitize_path_part,
};

use crate::cli::{DownloadCommand, Platform};
use crate::util::progress_bar;

pub struct ModEntry {
    pub id: String,
    pub slug: String,
    pub version_id: String,
}

pub fn download(cmd: DownloadCommand) -> Result<()> {
    let temp_path = cmd
        .temp_path
        .unwrap_or_else(|| env::temp_dir().join("packtrans-glossary"));
    let client = crate::util::http_client();

    match cmd.platform {
        Platform::Curseforge => {
            let file = cmd
                .list_file
                .as_ref()
                .context("--list-file is required for curseforge downloads")?;
            download_curseforge(&client, file, &cmd.output, &temp_path)
        }
        Platform::Modrinth => {
            let file = cmd
                .list_file
                .as_ref()
                .context("--list-file is required for modrinth downloads")?;
            download_modrinth(&client, file, &cmd.output, &temp_path)
        }
        Platform::Minecraft => download_minecraft(&client, &cmd.output, &temp_path),
    }
}

fn read_mod_list(path: &PathBuf) -> Result<Vec<ModEntry>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read mod list {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse mod list {}", path.display()))?;
    let items = json
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("mod list must be a JSON array"))?;

    let mut mods = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mod list item {index} is missing string field id"))?;
        let slug = item
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mod list item {index} is missing string field slug"))?;
        let version_id = item
            .get("version_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!("mod list item {index} is missing string field version_id")
            })?;

        mods.push(ModEntry {
            id: id.to_string(),
            slug: slug.to_string(),
            version_id: version_id.to_string(),
        });
    }

    Ok(mods)
}

fn download_curseforge(
    client: &ureq::Agent,
    file: &PathBuf,
    output: &Path,
    temp_path: &Path,
) -> Result<()> {
    let mods = read_mod_list(file)?;
    let pb = progress_bar(mods.len() as u64, "downloading CurseForge mods");
    let mut failures = Vec::new();

    for mod_entry in mods {
        pb.set_message(format!("downloading {}", mod_entry.slug));
        if let Err(err) = download_mod_jar_lang(
            client,
            "curseforge",
            &mod_entry.slug,
            &mod_entry.version_id,
            output,
            temp_path,
            &format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                mod_entry.id, mod_entry.version_id
            ),
        ) {
            eprintln!(
                "warning: failed to download CurseForge mod {}: {err:#}",
                mod_entry.slug
            );
            failures.push(mod_entry.slug);
        }
        pb.inc(1);
    }

    pb.finish_with_message("done");
    anyhow::ensure!(
        failures.is_empty(),
        "failed to download {} CurseForge mod(s): {}",
        failures.len(),
        failures.join(", ")
    );
    Ok(())
}

fn download_modrinth(
    client: &ureq::Agent,
    file: &PathBuf,
    output: &Path,
    temp_path: &Path,
) -> Result<()> {
    let mods = read_mod_list(file)?;
    let mut versions = HashMap::new();
    let version_ids: Vec<String> = mods.iter().map(|item| item.version_id.clone()).collect();
    if let Err(err) = fetch_modrinth_versions(client, &version_ids, &mut versions) {
        eprintln!("warning: failed to fetch Modrinth version metadata: {err:#}");
    }

    let pb = progress_bar(mods.len() as u64, "downloading Modrinth mods");
    let mut failures = Vec::new();
    for mod_entry in mods {
        pb.set_message(format!("downloading {}", mod_entry.slug));
        let Some(version) = versions.get(&mod_entry.version_id) else {
            eprintln!(
                "warning: missing Modrinth metadata for version {} ({})",
                mod_entry.version_id, mod_entry.slug
            );
            failures.push(mod_entry.slug.clone());
            pb.inc(1);
            continue;
        };

        let Some(url) = select_modrinth_jar_url(version) else {
            eprintln!(
                "warning: no downloadable jar for Modrinth mod {}",
                mod_entry.slug
            );
            failures.push(mod_entry.slug.clone());
            pb.inc(1);
            continue;
        };

        if let Err(err) = download_mod_jar_lang(
            client,
            "modrinth",
            &mod_entry.slug,
            &mod_entry.version_id,
            output,
            temp_path,
            url,
        )
        {
            eprintln!(
                "warning: failed to download Modrinth mod {}: {err:#}",
                mod_entry.slug
            );
            failures.push(mod_entry.slug);
        }
        pb.inc(1);
    }

    pb.finish_with_message("done");
    anyhow::ensure!(
        failures.is_empty(),
        "failed to download {} Modrinth mod(s): {}",
        failures.len(),
        failures.join(", ")
    );
    Ok(())
}

const MODRINTH_VERSION_CHUNK_SIZE: usize = 100;

fn fetch_modrinth_versions(
    client: &ureq::Agent,
    ids: &[String],
    versions: &mut HashMap<String, serde_json::Value>,
) -> Result<()> {
    for chunk in ids.chunks(MODRINTH_VERSION_CHUNK_SIZE) {
        if let Err(err) = fetch_modrinth_versions_chunk(client, chunk, versions) {
            eprintln!("warning: {err:#}");
        }
    }
    Ok(())
}

fn fetch_modrinth_versions_chunk(
    client: &ureq::Agent,
    ids: &[String],
    versions: &mut HashMap<String, serde_json::Value>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let ids_json = serde_json::to_string(ids)?;
    match client
        .get("https://api.modrinth.com/v2/versions")
        .query("ids", &ids_json)
        .call()
    {
        Ok(response) => {
            let json: serde_json::Value = response.into_json()?;
            let items = json
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Modrinth versions response must be an array"))?;
            for item in items {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    versions.insert(id.to_string(), item.clone());
                }
            }
            Ok(())
        }
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to fetch Modrinth metadata for chunk starting with {}",
                ids[0]
            )
        }),
    }
}

fn select_modrinth_jar_url(version: &serde_json::Value) -> Option<&str> {
    let files = version.get("files")?.as_array()?;
    files
        .iter()
        .find(|file| {
            file.get("primary").and_then(|v| v.as_bool()) == Some(true)
                && file
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| name.ends_with(".jar"))
        })
        .or_else(|| {
            files.iter().find(|file| {
                file.get("filename")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| name.ends_with(".jar"))
            })
        })
        .and_then(|file| file.get("url"))
        .and_then(|v| v.as_str())
}

fn download_mod_jar_lang(
    client: &ureq::Agent,
    platform: &str,
    slug: &str,
    version_id: &str,
    output: &Path,
    temp_path: &Path,
    url: &str,
) -> Result<()> {
    let slug = sanitize_path_part(slug);
    let version_id = sanitize_path_part(version_id);
    let temp_mods = temp_path.join("mods");
    let cache_key = format!("{platform}-{slug}-{version_id}");
    let jar_path = temp_mods.join(format!("{cache_key}.jar"));
    let extracted_dir = temp_mods.join(&cache_key);
    let output_dir = output.join(format!("{platform}-{slug}"));

    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)
            .with_context(|| format!("failed to clear {}", output_dir.display()))?;
    }

    if !jar_path.exists() {
        download_to_file(client, url, &jar_path).context("failed jar download")?;
    }
    extract_zip_file(&jar_path, &extracted_dir)?;
    let lang_dir = find_best_lang_dir(&extracted_dir).context("missing lang folder")?;
    copy_dir_contents(&lang_dir, &output_dir)?;
    // Keep jar_path as a download cache; drop the larger extracted tree after copying.
    if extracted_dir.exists() {
        fs::remove_dir_all(&extracted_dir).with_context(|| {
            format!(
                "failed to remove temp extract dir {}",
                extracted_dir.display()
            )
        })?;
    }
    Ok(())
}

fn download_minecraft(client: &ureq::Agent, output: &Path, temp_path: &Path) -> Result<()> {
    let manifest: serde_json::Value = client
        .get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
        .call()
        .context("failed Minecraft manifest fetch")?
        .into_json()
        .context("failed to parse Minecraft manifest")?;
    let latest_release = manifest
        .get("latest")
        .and_then(|v| v.get("release"))
        .and_then(|v| v.as_str())
        .context("Minecraft manifest missing latest.release")?;
    let version_url = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|versions| {
            versions.iter().find_map(|version| {
                (version.get("id").and_then(|v| v.as_str()) == Some(latest_release))
                    .then(|| version.get("url").and_then(|v| v.as_str()))
                    .flatten()
            })
        })
        .context("Minecraft manifest missing latest release metadata URL")?;

    let version: serde_json::Value = client
        .get(version_url)
        .call()
        .context("failed Minecraft version metadata fetch")?
        .into_json()
        .context("failed to parse Minecraft version metadata")?;

    let minecraft_output = output.join("minecraft");
    if minecraft_output.exists() {
        fs::remove_dir_all(&minecraft_output)
            .with_context(|| format!("failed to clear {}", minecraft_output.display()))?;
    }
    fs::create_dir_all(&minecraft_output)?;

    let client_url = version
        .get("downloads")
        .and_then(|v| v.get("client"))
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .context("Minecraft version metadata missing downloads.client.url")?;
    let client_jar = temp_path
        .join("minecraft")
        .join(format!("client-{}.jar", sanitize_path_part(latest_release)));
    if !client_jar.exists() {
        download_to_file(client, client_url, &client_jar).context("failed Minecraft client jar download")?;
    }
    extract_minecraft_en_us(&client_jar, &minecraft_output.join("en_us.json"))?;

    let asset_index_url = version
        .get("assetIndex")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .context("Minecraft version metadata missing assetIndex.url")?;
    let asset_index: serde_json::Value = client
        .get(asset_index_url)
        .call()
        .context("failed Minecraft asset index fetch")?
        .into_json()
        .context("failed to parse Minecraft asset index")?;
    let objects = asset_index
        .get("objects")
        .and_then(|v| v.as_object())
        .context("Minecraft asset index missing objects")?;

    let lang_objects: Vec<_> = objects
        .iter()
        .filter(|(key, _)| key.starts_with("minecraft/lang/") && key.ends_with(".json"))
        .collect();
    let pb = progress_bar(lang_objects.len() as u64, "downloading Minecraft assets");
    let mut failures = Vec::new();

    for (key, object) in lang_objects {
        let filename = key.rsplit('/').next().unwrap_or(key);
        if filename == "en_us.json" && minecraft_output.join(filename).exists() {
            pb.inc(1);
            continue;
        }

        let Some(hash) = object.get("hash").and_then(|v| v.as_str()) else {
            eprintln!("warning: Minecraft asset {key} is missing hash");
            failures.push(key.to_string());
            pb.inc(1);
            continue;
        };
        if hash.len() < 2 {
            eprintln!("warning: Minecraft asset {key} has invalid hash {hash}");
            failures.push(key.to_string());
            pb.inc(1);
            continue;
        }

        let url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            &hash[..2],
            hash
        );
        if let Err(err) = download_to_file(client, &url, &minecraft_output.join(filename)) {
            eprintln!("warning: failed to download Minecraft asset {key}: {err:#}");
            failures.push(key.to_string());
        }
        pb.inc(1);
    }

    pb.finish_with_message("done");
    anyhow::ensure!(
        failures.is_empty(),
        "failed to download {} Minecraft asset(s): {}",
        failures.len(),
        failures.join(", ")
    );
    Ok(())
}

fn extract_minecraft_en_us(jar_path: &Path, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file =
        fs::File::open(jar_path).with_context(|| format!("failed to open {}", jar_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("failed to read zip archive {}", jar_path.display()))?;
    let mut entry = archive
        .by_name("assets/minecraft/lang/en_us.json")
        .context("Minecraft client jar missing assets/minecraft/lang/en_us.json")?;
    let mut output = fs::File::create(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    std::io::copy(&mut entry, &mut output)?;
    Ok(())
}
