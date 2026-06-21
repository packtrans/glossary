use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use packtrans_glossary_core::util;
use packtrans_glossary_core::{index_meta_path, indexes_root, lang_index_dir, release_index_dir};
use serde_json::json;

use crate::progress;

const GLOSSARY_INDEXES_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/packtrans/glossary-indexes/releases/latest";
const INDEX_METADATA_FILE: &str = ".packtrans-glossary-index.json";
const MAX_RELEASE_BODY_BYTES: usize = 2 * 1024 * 1024;
const VERSION_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Subcommand)]
pub enum IndexCommand {
    /// Download the latest release index for a language.
    Download(IndexDownloadCommand),
    /// Download the latest release index and remove older versions.
    Upgrade(IndexDownloadCommand),
    /// List installed release indexes.
    Ls,
    /// Delete a downloaded index.
    Delete(IndexDeleteCommand),
    /// Remove old index versions.
    Clean(IndexCleanCommand),
}

#[derive(Args)]
pub struct IndexDownloadCommand {
    /// Target language code (e.g. `zh_cn`, `ja_jp`).
    #[arg(long)]
    pub lang: String,
}

#[derive(Args)]
pub struct IndexDeleteCommand {
    /// Target language code to delete.
    #[arg(long)]
    pub lang: String,
    /// Index release version to delete (defaults to latest installed).
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Args)]
pub struct IndexCleanCommand {
    /// Release version to keep (defaults to current or latest installed).
    #[arg(long)]
    pub keep_version: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DownloadedIndexMeta {
    latest_version_check_time: Option<u64>,
    current_version: Option<String>,
    current_version_downloaded_time: Option<u64>,
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

pub fn run(command: IndexCommand) -> Result<()> {
    match command {
        IndexCommand::Download(cmd) => {
            let entry = download_latest(&cmd.lang, None, false)?;
            println!(
                "{}@{} -> {}",
                entry.lang,
                entry.version,
                entry.path.display()
            );
            Ok(())
        }
        IndexCommand::Upgrade(cmd) => {
            let entry = download_latest(&cmd.lang, None, true)?;
            println!(
                "{}@{} -> {}",
                entry.lang,
                entry.version,
                entry.path.display()
            );
            Ok(())
        }
        IndexCommand::Ls => list(None),
        IndexCommand::Delete(cmd) => delete(cmd, None),
        IndexCommand::Clean(cmd) => clean(cmd, None),
    }
}

pub fn resolve_query_index_dir(lang: &str, index_dir: Option<&Path>) -> Result<PathBuf> {
    util::validate_path_segment(lang, "lang")?;
    if let Some(base) = index_dir {
        let path = lang_index_dir(base, lang)?;
        if !path.is_dir() {
            bail!("index directory does not exist: {}", path.display());
        }
        return Ok(path);
    }

    let path = resolve_downloaded_index_dir(lang, None, false)?;
    if let Err(e) = clean_old_versions_from_meta(None) {
        eprintln!("warning: failed to clean old versions: {}", e);
    }
    Ok(path)
}

fn resolve_downloaded_index_dir(
    lang: &str,
    base: Option<&Path>,
    force_version_check: bool,
) -> Result<PathBuf> {
    let root = indexes_root_or(base)?;
    resolve_downloaded_index_dir_inner(lang, base, force_version_check, &root)
}

fn resolve_downloaded_index_dir_inner(
    lang: &str,
    base: Option<&Path>,
    force_version_check: bool,
    root: &Path,
) -> Result<PathBuf> {
    let mut meta = read_downloaded_meta(root)?;
    let now = unix_now();

    let checked_release = if force_version_check || should_check_latest_version(&meta, now) {
        let release = fetch_latest_release()?;
        meta.latest_version_check_time = Some(now);
        write_downloaded_meta(root, &meta)?;
        Some(release)
    } else {
        None
    };

    let version = if let Some(release) = &checked_release {
        release.tag_name.as_str()
    } else {
        meta.current_version
            .as_deref()
            .ok_or_else(|| anyhow!("no downloaded index version recorded in meta.json"))?
    };

    let index_dir = release_index_dir(root, version, lang)?;
    if index_dir.is_dir() {
        if meta.current_version.as_deref() != Some(version) {
            meta.current_version = Some(version.to_string());
            write_downloaded_meta(root, &meta)?;
        }
        return Ok(index_dir);
    }

    let release = match checked_release {
        Some(release) => release,
        None => fetch_latest_release()?,
    };

    let entry = ensure_release_index(lang, base, &release)?;
    meta.current_version = Some(release.tag_name.clone());
    meta.current_version_downloaded_time = Some(now);
    meta.latest_version_check_time = Some(now);
    write_downloaded_meta(root, &meta)?;
    Ok(entry.path)
}

fn download_latest(lang: &str, base: Option<&Path>, clean_old: bool) -> Result<IndexEntry> {
    let release = fetch_latest_release()?;
    let root = indexes_root_or(base)?;
    let now = unix_now();
    let mut meta = read_downloaded_meta(&root)?;
    meta.latest_version_check_time = Some(now);
    write_downloaded_meta(&root, &meta)?;

    let entry = ensure_release_index(lang, base, &release)?;
    meta.current_version = Some(release.tag_name.clone());
    meta.current_version_downloaded_time = Some(now);
    meta.latest_version_check_time = Some(now);
    write_downloaded_meta(&root, &meta)?;

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
    util::validate_path_segment(&cmd.lang, "lang")?;
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
        None => resolve_keep_version_for_clean(base)?,
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

fn fetch_latest_release() -> Result<Release> {
    let pb = progress::spinner("Checking latest glossary index release");
    let result = fetch_latest_release_inner();
    pb.finish_and_clear();
    result
}

fn fetch_latest_release_inner() -> Result<Release> {
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
    util::validate_path_segment(lang, "lang")?;
    let root = indexes_root_or(base)?;
    let index_dir = release_index_dir(&root, &release.tag_name, lang)?;
    if index_dir.is_dir() {
        return Ok(IndexEntry {
            lang: lang.to_string(),
            version: release.tag_name.clone(),
            path: index_dir,
        });
    }

    let asset = select_asset(release, lang)?;
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
    let version_dir = root.join(&release.tag_name);
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
    if install_result.is_err() && temp_index_dir.exists() {
        let _ = fs::remove_dir_all(&temp_index_dir);
    }
    install_result
}

pub fn list_downloaded_indexes(base: Option<&Path>) -> Result<Vec<IndexEntry>> {
    let root = indexes_root_or(base)?;
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
        if is_reserved_version_dir(&version) {
            continue;
        }
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
    util::validate_path_segment(lang, "lang")?;
    Ok(list_downloaded_indexes(base)?
        .into_iter()
        .rfind(|entry| entry.lang == lang))
}

fn delete_downloaded_index(lang: &str, version: &str, base: Option<&Path>) -> Result<()> {
    let root = indexes_root_or(base)?;
    let index_dir = release_index_dir(&root, version, lang)?;
    if !index_dir.is_dir() {
        bail!("downloaded index not found: {}", index_dir.display());
    }
    let version_dir = index_dir
        .parent()
        .ok_or_else(|| anyhow!("invalid downloaded index path: {}", index_dir.display()))?;
    fs::remove_dir_all(&index_dir)
        .with_context(|| format!("failed to delete {}", index_dir.display()))?;
    remove_dir_if_empty(version_dir)?;

    let mut meta = read_downloaded_meta(&root)?;
    if meta.current_version.as_deref() == Some(version) && !version_dir_has_indexes(version_dir)? {
        meta.current_version = latest_installed_version(base);
        write_downloaded_meta(&root, &meta)?;
    }
    Ok(())
}

fn clean_old_versions_from_meta(base: Option<&Path>) -> Result<Vec<String>> {
    let root = indexes_root_or(base)?;
    let meta = read_downloaded_meta(&root)?;
    let Some(keep_version) = meta.current_version else {
        return Ok(vec![]);
    };
    clean_old_versions_keep(base, &keep_version)
}

fn clean_old_versions_keep(base: Option<&Path>, keep_version: &str) -> Result<Vec<String>> {
    util::validate_path_segment(keep_version, "release tag")?;
    let root = indexes_root_or(base)?;
    if !root.is_dir() {
        return Ok(vec![]);
    }
    if !root.join(keep_version).is_dir() {
        bail!("keep version {keep_version} is not installed");
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
        if is_reserved_version_dir(&version) || version == keep_version {
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
    util::validate_path_segment(lang, "lang")?;
    let prefix = format!("packtrans-glossary-index-{lang}-");
    release
        .assets
        .iter()
        .find(|asset| asset.name.starts_with(&prefix) && asset.name.ends_with(".zip"))
        .ok_or_else(|| {
            let available = match available_languages(release) {
                Ok(langs) if langs.is_empty() => "none".to_string(),
                Ok(langs) => langs.join(", "),
                Err(err) => format!("unknown ({err})"),
            };
            anyhow!(
                "release {} has no index asset for {}. Available languages: {}",
                release.tag_name,
                lang,
                available
            )
        })
}

fn available_languages(release: &Release) -> Result<Vec<String>> {
    fetch_available_languages(&release.tag_name)
}

fn fetch_available_languages(tag: &str) -> Result<Vec<String>> {
    util::validate_path_segment(tag, "release tag")?;
    let url = format!(
        "https://raw.githubusercontent.com/packtrans/glossary-indexes/refs/tags/{tag}/languages.json"
    );
    let response = http_client()
        .get(&url)
        .call()
        .with_context(|| format!("failed to fetch available languages from {url}"))?;
    let mut body = String::new();
    response
        .into_reader()
        .take((MAX_RELEASE_BODY_BYTES + 1) as u64)
        .read_to_string(&mut body)
        .with_context(|| format!("failed to read available languages from {url}"))?;
    if body.len() > MAX_RELEASE_BODY_BYTES {
        bail!("available languages response exceeded {MAX_RELEASE_BODY_BYTES} bytes");
    }

    let value: serde_json::Value =
        serde_json::from_str(&body).context("failed to parse available languages")?;
    let langs = match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        serde_json::Value::Object(map) => map
            .get("languages")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    if langs.is_empty() {
        bail!("available languages response did not include any languages");
    }
    Ok(langs)
}

fn resolve_keep_version_for_clean(base: Option<&Path>) -> Result<String> {
    let root = indexes_root_or(base)?;
    if let Some(version) = read_downloaded_meta(&root)?.current_version {
        return Ok(version);
    }
    if let Some(version) = latest_installed_version(base) {
        return Ok(version);
    }
    Ok(fetch_latest_release()?.tag_name)
}

fn latest_installed_version(base: Option<&Path>) -> Option<String> {
    list_downloaded_indexes(base)
        .ok()?
        .into_iter()
        .map(|entry| entry.version)
        .max()
}

fn indexes_root_or(base: Option<&Path>) -> Result<PathBuf> {
    match base {
        Some(path) => Ok(path.to_path_buf()),
        None => indexes_root(),
    }
}

fn is_reserved_version_dir(name: &str) -> bool {
    name.starts_with('.')
}

fn version_dir_has_indexes(version_dir: &Path) -> Result<bool> {
    if !version_dir.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(version_dir)
        .with_context(|| format!("failed to read directory {}", version_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_downloaded_meta(index_root: &Path) -> Result<DownloadedIndexMeta> {
    let path = index_meta_path(index_root);
    if !path.is_file() {
        return Ok(DownloadedIndexMeta::default());
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    Ok(DownloadedIndexMeta {
        latest_version_check_time: value
            .get("latest_version_check_time")
            .and_then(serde_json::Value::as_u64),
        current_version: value
            .get("current_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        current_version_downloaded_time: value
            .get("current_version_downloaded_time")
            .and_then(serde_json::Value::as_u64),
    })
}

static META_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_downloaded_meta(index_root: &Path, meta: &DownloadedIndexMeta) -> Result<()> {
    let path = index_meta_path(index_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let value = json!({
        "latest_version_check_time": meta.latest_version_check_time,
        "current_version": meta.current_version,
        "current_version_downloaded_time": meta.current_version_downloaded_time,
    });
    let bytes = serde_json::to_vec_pretty(&value)?;
    let write_id = META_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = path.with_file_name(format!(
        "{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("meta.json"),
        std::process::id(),
        write_id
    ));
    fs::write(&temp_path, &bytes)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    let rename_result = fs::rename(&temp_path, &path).with_context(|| {
        format!(
            "failed to move {} to {}",
            temp_path.display(),
            path.display()
        )
    });
    if rename_result.is_err() && temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }
    rename_result?;
    Ok(())
}

fn should_check_latest_version(meta: &DownloadedIndexMeta, now: u64) -> bool {
    if meta.current_version.is_none() {
        return true;
    }
    match meta.latest_version_check_time {
        None => true,
        Some(last_check) => now.saturating_sub(last_check) >= VERSION_CHECK_INTERVAL.as_secs(),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
    use std::sync::{Arc, Barrier};
    use std::thread;

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
        fs::create_dir_all(root.join("index-20260525/zh_cn")).unwrap();
        fs::create_dir_all(root.join("index-20260526/ja_jp")).unwrap();

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
    fn clean_rejects_missing_keep_version() {
        let root = temp_root("clean-missing-keep");
        fs::create_dir_all(root.join("index-20260525/zh_cn")).unwrap();

        let err = clean_old_versions_keep(Some(&root), "index-20260599").unwrap_err();
        assert!(err.to_string().contains("not installed"));
        assert!(root.join("index-20260525/zh_cn").is_dir());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_clears_meta_when_active_version_removed() {
        let root = temp_root("delete-meta");
        fs::create_dir_all(root.join("index-20260526/zh_cn")).unwrap();
        fs::write(
            index_meta_path(&root),
            r#"{"current_version":"index-20260526"}"#,
        )
        .unwrap();

        delete_downloaded_index("zh_cn", "index-20260526", Some(&root)).unwrap();
        let meta = read_downloaded_meta(&root).unwrap();
        assert!(meta.current_version.is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cleans_versions_except_keep_version() {
        let root = temp_root("clean-indexes");
        fs::create_dir_all(root.join("index-20260525/zh_cn")).unwrap();
        fs::create_dir_all(root.join("index-20260526/zh_cn")).unwrap();

        let removed = clean_old_versions_keep(Some(&root), "index-20260526").unwrap();
        assert_eq!(removed, vec!["index-20260525"]);
        assert!(!root.join("index-20260525").exists());
        assert!(root.join("index-20260526/zh_cn").is_dir());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn version_check_is_throttled_for_one_day() {
        let meta = DownloadedIndexMeta {
            latest_version_check_time: Some(1_000),
            current_version: Some("index-20260526".to_string()),
            ..Default::default()
        };
        assert!(!should_check_latest_version(
            &meta,
            1_000 + VERSION_CHECK_INTERVAL.as_secs() - 1
        ));
        assert!(should_check_latest_version(
            &meta,
            1_000 + VERSION_CHECK_INTERVAL.as_secs()
        ));

        let no_version = DownloadedIndexMeta {
            latest_version_check_time: Some(1_000),
            ..Default::default()
        };
        assert!(should_check_latest_version(
            &no_version,
            1_000 + VERSION_CHECK_INTERVAL.as_secs() - 1
        ));
    }

    #[test]
    fn concurrent_meta_writes_succeed() {
        let root = temp_root("concurrent-meta-write");
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for i in 0..8 {
            let barrier = Arc::clone(&barrier);
            let root = root.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                let mut meta = read_downloaded_meta(&root).unwrap();
                meta.latest_version_check_time = Some(i);
                write_downloaded_meta(&root, &meta).unwrap();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(index_meta_path(&root).is_file());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn uses_explicit_index_dir_when_provided() {
        let root = temp_root("explicit-index-dir");
        let index_root = root.join("indexes");
        let local_index = index_root.join("zh_cn");
        fs::create_dir_all(&local_index).unwrap();

        let resolved = resolve_query_index_dir("zh_cn", Some(&index_root)).unwrap();
        assert_eq!(resolved, local_index);

        let _ = fs::remove_dir_all(&root);
    }
}
