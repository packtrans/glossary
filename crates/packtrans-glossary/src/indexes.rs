use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use packtrans_glossary_core::{indexes_root, util, validate_lang};
use serde_json::json;

use crate::progress;

const GLOSSARY_INDEXES_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/packtrans/glossary-indexes/releases/latest";
const DOWNLOADED_INDEXES_DIR: &str = ".downloaded";
const INDEX_METADATA_FILE: &str = ".packtrans-glossary-index.json";
const MAX_RELEASE_BODY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Subcommand)]
pub enum IndexCommand {
    Download(IndexDownloadCommand),
    Upgrade(IndexDownloadCommand),
    Ls,
    Delete(IndexDeleteCommand),
    Clean(IndexCleanCommand),
}

#[derive(Args)]
pub struct IndexDownloadCommand {
    #[arg(long)]
    pub lang: String,
}

#[derive(Args)]
pub struct IndexDeleteCommand {
    #[arg(long)]
    pub lang: String,
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Args)]
pub struct IndexCleanCommand {
    #[arg(long)]
    pub keep_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexEntry {
    pub lang: String,
    pub version: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Release {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub fn run(command: IndexCommand, base: Option<&Path>) -> Result<()> {
    match command {
        IndexCommand::Download(cmd) => {
            let entry = download_latest(&cmd.lang, base, false)?;
            println!(
                "{}@{} -> {}",
                entry.lang,
                entry.version,
                entry.path.display()
            );
            Ok(())
        }
        IndexCommand::Upgrade(cmd) => {
            let entry = download_latest(&cmd.lang, base, true)?;
            println!(
                "{}@{} -> {}",
                entry.lang,
                entry.version,
                entry.path.display()
            );
            Ok(())
        }
        IndexCommand::Ls => list(base),
        IndexCommand::Delete(cmd) => delete(cmd, base),
        IndexCommand::Clean(cmd) => clean(cmd, base),
    }
}

pub fn resolve_query_index_dir(lang: &str, base: Option<&Path>) -> Result<PathBuf> {
    validate_lang(lang)?;
    let root = indexes_root_or(base)?;
    let local_index_dir = root.join(lang);
    if local_index_dir.is_dir() {
        return Ok(local_index_dir);
    }

    match latest_release() {
        Ok(release) => {
            match ensure_release_index(lang, base, &release) {
                Ok(entry) => {
                    if let Err(e) = clean_old_versions_keep(base, &release.tag_name) {
                        eprintln!("warning: failed to clean old versions: {}", e);
                    }
                    Ok(entry.path)
                }
                Err(install_err) => {
                    if let Some(entry) = latest_installed_for_lang(lang, base)? {
                        eprintln!(
                            "warning: failed to download latest index release ({install_err}); using installed {}@{}",
                            entry.lang, entry.version
                        );
                        return Ok(entry.path);
                    }
                    Err(install_err).context("failed to download latest index release and no local index is installed")
                }
            }
        }
        Err(err) => {
            if let Some(entry) = latest_installed_for_lang(lang, base)? {
                eprintln!(
                    "warning: failed to check latest index release ({err}); using installed {}@{}",
                    entry.lang, entry.version
                );
                return Ok(entry.path);
            }
            Err(err).context("failed to check latest index release and no local index is installed")
        }
    }
}

fn download_latest(lang: &str, base: Option<&Path>, clean_old: bool) -> Result<IndexEntry> {
    let release = latest_release()?;
    let entry = ensure_release_index(lang, base, &release)?;
    if clean_old {
        for version in clean_old_versions_keep(base, &release.tag_name)? {
            println!("removed version {}", version);
        }
    }
    Ok(entry)
}

fn list(base: Option<&Path>) -> Result<()> {
    let entries = list_downloaded_indexes(base)?;
    if entries.is_empty() {
        println!("no downloaded indexes installed");
        return Ok(());
    }

    println!("{:<12} {:<16} PATH", "LANG", "VERSION");
    for entry in entries {
        println!(
            "{:<12} {:<16} {}",
            entry.lang,
            entry.version,
            entry.path.display()
        );
    }
    Ok(())
}

fn delete(cmd: IndexDeleteCommand, base: Option<&Path>) -> Result<()> {
    validate_lang(&cmd.lang)?;
    let version = match cmd.version {
        Some(version) => version,
        None => latest_installed_for_lang(&cmd.lang, base)?
            .map(|entry| entry.version)
            .ok_or_else(|| anyhow!("no downloaded index installed for {}", cmd.lang))?,
    };
    delete_downloaded_index(&cmd.lang, &version, base)?;
    println!("deleted {}@{}", cmd.lang, version);
    Ok(())
}

fn clean(cmd: IndexCleanCommand, base: Option<&Path>) -> Result<()> {
    let keep_version = match cmd.keep_version {
        Some(version) => version,
        None => latest_release()?.tag_name,
    };
    let removed = clean_old_versions_keep(base, &keep_version)?;
    if removed.is_empty() {
        println!("no old versions to clean");
    } else {
        for version in &removed {
            println!("removed version {}", version);
        }
    }
    Ok(())
}

fn latest_release() -> Result<Release> {
    let pb = progress::spinner("Checking latest glossary index release");
    let result = fetch_latest_release();
    pb.finish_and_clear();
    result
}

fn fetch_latest_release() -> Result<Release> {
    let response = http_client()
        .get(GLOSSARY_INDEXES_LATEST_RELEASE_URL)
        .call()
        .context("failed to fetch latest glossary index release")?;
    let mut body = String::new();
    response
        .into_reader()
        .take((MAX_RELEASE_BODY_BYTES + 1) as u64)
        .read_to_string(&mut body)
        .context("failed to read latest glossary index release response")?;
    if body.len() > MAX_RELEASE_BODY_BYTES {
        bail!(
            "latest glossary index release response exceeded {} bytes",
            MAX_RELEASE_BODY_BYTES
        );
    }

    let value: serde_json::Value =
        serde_json::from_str(&body).context("failed to parse latest glossary index release")?;
    let tag_name = value
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("latest glossary index release did not include tag_name"))?
        .to_string();
    util::validate_path_segment(&tag_name, "release tag")?;

    let assets = value
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("latest glossary index release did not include assets"))?
        .iter()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?.to_string();
            let browser_download_url = asset.get("browser_download_url")?.as_str()?.to_string();
            Some(ReleaseAsset {
                name,
                browser_download_url,
            })
        })
        .collect();

    Ok(Release { tag_name, assets })
}

fn ensure_release_index(lang: &str, base: Option<&Path>, release: &Release) -> Result<IndexEntry> {
    validate_lang(lang)?;
    let asset = select_asset(release, lang)?;
    let index_dir = downloaded_index_dir(base, &release.tag_name, lang)?;
    if index_dir.is_dir() {
        return Ok(IndexEntry {
            lang: lang.to_string(),
            version: release.tag_name.clone(),
            path: index_dir,
        });
    }

    let pb = progress::spinner(format!("Downloading {lang} index {}", release.tag_name));
    let result = install_asset(lang, base, release, asset);
    pb.finish_and_clear();
    result
}

fn install_asset(
    lang: &str,
    base: Option<&Path>,
    release: &Release,
    asset: &ReleaseAsset,
) -> Result<IndexEntry> {
    let root = indexes_root_or(base)?;
    let downloaded_root = downloaded_indexes_root(base)?;
    let version_dir = downloaded_root.join(&release.tag_name);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("failed to create {}", version_dir.display()))?;

    let work_dir = root.join(".tmp").join(format!(
        "{}-{}-{}",
        release.tag_name,
        lang,
        std::process::id()
    ));
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)
            .with_context(|| format!("failed to clear {}", work_dir.display()))?;
    }
    fs::create_dir_all(&work_dir)
        .with_context(|| format!("failed to create {}", work_dir.display()))?;

    let zip_path = work_dir.join(&asset.name);
    let extract_dir = work_dir.join("extract");
    let temp_index_dir = version_dir.join(format!(".{lang}.tmp-{}", std::process::id()));
    let final_index_dir = version_dir.join(lang);

    let install_result = (|| {
        util::download_to_file(
            &http_download_client(),
            &asset.browser_download_url,
            &zip_path,
        )?;
        util::extract_zip_file(&zip_path, &extract_dir)?;

        let extracted_index_dir = extract_dir.join(lang);
        if !extracted_index_dir.is_dir() {
            bail!(
                "downloaded index archive did not contain expected {} directory",
                lang
            );
        }

        if temp_index_dir.exists() {
            fs::remove_dir_all(&temp_index_dir)
                .with_context(|| format!("failed to clear {}", temp_index_dir.display()))?;
        }
        fs::rename(&extracted_index_dir, &temp_index_dir).with_context(|| {
            format!(
                "failed to move {} to {}",
                extracted_index_dir.display(),
                temp_index_dir.display()
            )
        })?;

        let metadata = json!({
            "repository": "packtrans/glossary-indexes",
            "tag_name": &release.tag_name,
            "asset_name": &asset.name,
            "browser_download_url": &asset.browser_download_url,
        });
        fs::write(
            temp_index_dir.join(INDEX_METADATA_FILE),
            serde_json::to_vec_pretty(&metadata)?,
        )
        .with_context(|| {
            format!(
                "failed to write {}",
                temp_index_dir.join(INDEX_METADATA_FILE).display()
            )
        })?;

        match fs::rename(&temp_index_dir, &final_index_dir) {
            Ok(()) => {}
            Err(rename_err) => {
                if final_index_dir.exists() {
                    // Concurrent installation succeeded; clean up our temp directory
                    let _ = fs::remove_dir_all(&temp_index_dir);
                } else {
                    return Err(rename_err).with_context(|| {
                        format!(
                            "failed to move {} to {}",
                            temp_index_dir.display(),
                            final_index_dir.display()
                        )
                    });
                }
            }
        }

        Ok(IndexEntry {
            lang: lang.to_string(),
            version: release.tag_name.clone(),
            path: final_index_dir,
        })
    })();

    let _ = fs::remove_dir_all(&work_dir);
    install_result
}

pub fn list_downloaded_indexes(base: Option<&Path>) -> Result<Vec<IndexEntry>> {
    let root = downloaded_indexes_root(base)?;
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for version_entry in fs::read_dir(&root)
        .with_context(|| format!("failed to read directory {}", root.display()))?
    {
        let version_entry = version_entry?;
        if !version_entry.file_type()?.is_dir() {
            continue;
        }
        let version = version_entry.file_name().to_string_lossy().into_owned();
        for lang_entry in fs::read_dir(version_entry.path()).with_context(|| {
            format!(
                "failed to read directory {}",
                version_entry.path().display()
            )
        })? {
            let lang_entry = lang_entry?;
            if !lang_entry.file_type()?.is_dir() {
                continue;
            }
            let lang = lang_entry.file_name().to_string_lossy().into_owned();
            if lang.starts_with('.') {
                continue;
            }
            entries.push(IndexEntry {
                lang,
                version: version.clone(),
                path: lang_entry.path(),
            });
        }
    }

    entries.sort_by(|a, b| (&a.version, &a.lang).cmp(&(&b.version, &b.lang)));
    Ok(entries)
}

fn latest_installed_for_lang(lang: &str, base: Option<&Path>) -> Result<Option<IndexEntry>> {
    validate_lang(lang)?;
    Ok(list_downloaded_indexes(base)?
        .into_iter()
        .rfind(|entry| entry.lang == lang))
}

fn delete_downloaded_index(lang: &str, version: &str, base: Option<&Path>) -> Result<()> {
    validate_lang(lang)?;
    util::validate_path_segment(version, "release tag")?;
    let version_dir = downloaded_indexes_root(base)?.join(version);
    let index_dir = version_dir.join(lang);
    if !index_dir.is_dir() {
        bail!("downloaded index not found: {}", index_dir.display());
    }
    fs::remove_dir_all(&index_dir)
        .with_context(|| format!("failed to delete {}", index_dir.display()))?;
    remove_dir_if_empty(&version_dir)?;
    Ok(())
}

fn clean_old_versions_keep(base: Option<&Path>, keep_version: &str) -> Result<Vec<String>> {
    util::validate_path_segment(keep_version, "release tag")?;
    let root = downloaded_indexes_root(base)?;
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let mut removed = Vec::new();
    for entry in fs::read_dir(&root)
        .with_context(|| format!("failed to read directory {}", root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let version = entry.file_name().to_string_lossy().into_owned();
        if version == keep_version {
            continue;
        }
        fs::remove_dir_all(entry.path())
            .with_context(|| format!("failed to delete {}", entry.path().display()))?;
        removed.push(version);
    }

    removed.sort();
    Ok(removed)
}

fn select_asset<'a>(release: &'a Release, lang: &str) -> Result<&'a ReleaseAsset> {
    validate_lang(lang)?;
    let prefix = format!("packtrans-glossary-index-{lang}-");
    release
        .assets
        .iter()
        .find(|asset| asset.name.starts_with(&prefix) && asset.name.ends_with(".zip"))
        .ok_or_else(|| {
            let available = available_languages(release).join(", ");
            anyhow!(
                "release {} has no index asset for {}. Available languages: {}",
                release.tag_name,
                lang,
                if available.is_empty() {
                    "none".to_string()
                } else {
                    available
                }
            )
        })
}

fn available_languages(release: &Release) -> Vec<String> {
    let mut langs = release
        .assets
        .iter()
        .filter_map(|asset| asset.name.strip_prefix("packtrans-glossary-index-"))
        .filter_map(|rest| rest.rsplit_once('-').map(|(lang, _)| lang.to_string()))
        .collect::<Vec<_>>();
    langs.sort();
    langs.dedup();
    langs
}

fn indexes_root_or(base: Option<&Path>) -> Result<PathBuf> {
    match base {
        Some(path) => Ok(path.to_path_buf()),
        None => indexes_root(),
    }
}

fn downloaded_indexes_root(base: Option<&Path>) -> Result<PathBuf> {
    Ok(indexes_root_or(base)?.join(DOWNLOADED_INDEXES_DIR))
}

fn downloaded_index_dir(base: Option<&Path>, version: &str, lang: &str) -> Result<PathBuf> {
    util::validate_path_segment(version, "release tag")?;
    validate_lang(lang)?;
    Ok(downloaded_indexes_root(base)?.join(version).join(lang))
}

fn remove_dir_if_empty(path: &Path) -> Result<()> {
    if path.is_dir() && path.read_dir()?.next().is_none() {
        fs::remove_dir(path).with_context(|| format!("failed to delete {}", path.display()))?;
    }
    Ok(())
}

fn http_client() -> ureq::Agent {
    agent_builder()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(30))
        .build()
}

fn http_download_client() -> ureq::Agent {
    agent_builder()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(600))
        .build()
}

fn agent_builder() -> ureq::AgentBuilder {
    let user_agent = format!(
        "packtrans/glossary/{} (https://github.com/packtrans/glossary)",
        env!("CARGO_PKG_VERSION")
    );
    ureq::AgentBuilder::new().user_agent(&user_agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("packtrans-glossary-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn selects_matching_language_asset() {
        let release = Release {
            tag_name: "index-20260526".to_string(),
            assets: vec![
                ReleaseAsset {
                    name: "packtrans-glossary-index-ja_jp-20260526.zip".to_string(),
                    browser_download_url: "https://example.test/ja.zip".to_string(),
                },
                ReleaseAsset {
                    name: "packtrans-glossary-index-zh_cn-20260526.zip".to_string(),
                    browser_download_url: "https://example.test/zh.zip".to_string(),
                },
            ],
        };

        let asset = select_asset(&release, "zh_cn").unwrap();
        assert_eq!(asset.browser_download_url, "https://example.test/zh.zip");
    }

    #[test]
    fn lists_downloaded_indexes_by_version_and_language() {
        let root = temp_root("list-indexes");
        fs::create_dir_all(root.join(".downloaded/index-20260525/zh_cn")).unwrap();
        fs::create_dir_all(root.join(".downloaded/index-20260526/ja_jp")).unwrap();

        let entries = list_downloaded_indexes(Some(&root)).unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.version.as_str(), entry.lang.as_str()))
                .collect::<Vec<_>>(),
            vec![("index-20260525", "zh_cn"), ("index-20260526", "ja_jp")]
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cleans_versions_except_keep_version() {
        let root = temp_root("clean-indexes");
        fs::create_dir_all(root.join(".downloaded/index-20260525/zh_cn")).unwrap();
        fs::create_dir_all(root.join(".downloaded/index-20260526/zh_cn")).unwrap();

        let removed = clean_old_versions_keep(Some(&root), "index-20260526").unwrap();
        assert_eq!(removed, vec!["index-20260525"]);
        assert!(!root.join(".downloaded/index-20260525").exists());
        assert!(root.join(".downloaded/index-20260526/zh_cn").is_dir());

        let _ = fs::remove_dir_all(&root);
    }
}
