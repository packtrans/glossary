use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub const IPADIC: &str = "lindera-ipadic";
pub const KO_DIC: &str = "lindera-ko-dic";
pub const JIEBA: &str = "lindera-jieba";

pub const DICTIONARY_NAMES: &[&str] = &[IPADIC, KO_DIC, JIEBA];

const MAX_REMOTE_ZIP_BYTES: usize = 50 * 1024 * 1024;

/// An entry representing an installed dictionary.
pub struct DictEntry {
    /// Name of the dictionary (e.g. `lindera-ipadic`).
    pub name: String,
    /// Version of the dictionary.
    pub version: String,
    /// Absolute path to the dictionary directory.
    pub path: PathBuf,
}

/// Returns the version of the underlying `lindera` library.
pub fn current_version() -> &'static str {
    lindera::get_version()
}

/// Returns the platform-specific data directory.
///
/// - macOS: `~/Library/Application Support`
/// - Windows: `%LOCALAPPDATA%`
/// - Linux/Other: `$XDG_DATA_HOME` or `~/.local/share`
pub fn data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join("Library/Application Support"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("LOCALAPPDATA environment variable not set")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                let home = std::env::var("HOME").context("HOME environment variable not set")?;
                Ok(PathBuf::from(home).join(".local/share"))
            })
    }
}

/// Returns the root directory where dictionaries are stored.
fn dictionaries_root() -> Result<PathBuf> {
    Ok(data_dir()?.join("packtrans-glossary").join("dictionaries"))
}

/// Resolves the dictionary root, using `base` if provided or falling back to [`dictionaries_root`].
fn dictionaries_root_or(base: Option<&Path>) -> Result<PathBuf> {
    match base {
        Some(p) => Ok(p.to_path_buf()),
        None => dictionaries_root(),
    }
}

/// Validates that `s` is a non-empty path segment without directory traversal.
fn validate_segment(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("segment must not be empty");
    }
    if s.contains("..") || s.contains('/') || s.contains('\\') {
        bail!("segment contains invalid path component: {}", s);
    }
    Ok(())
}

/// Ensures a dictionary is available locally, downloading it if necessary.
///
/// Returns the path to the dictionary directory.
pub fn ensure_dictionary(name: &str, base: Option<&Path>) -> Result<PathBuf> {
    let version = lindera::get_version();
    validate_segment(name)?;
    validate_segment(version)?;
    let root = dictionaries_root_or(base)?;
    let dict_dir = root.join(version).join(name);
    if dict_dir.is_dir() {
        return Ok(dict_dir);
    }
    let url = format!(
        "https://github.com/lindera/lindera/releases/download/v{}/{}-{}.zip",
        version, name, version
    );

    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("failed to download {} dictionary from {}", name, url))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take((MAX_REMOTE_ZIP_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {} dictionary response body", name))?;
    if bytes.len() > MAX_REMOTE_ZIP_BYTES {
        bail!(
            "download of {} dictionary exceeded the configured max size of {} bytes",
            name,
            MAX_REMOTE_ZIP_BYTES,
        );
    }

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .with_context(|| format!("failed to parse {} dictionary zip archive", name))?;

    let version_dir = root.join(version);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("failed to create directory {}", version_dir.display()))?;

    archive
        .extract(&version_dir)
        .with_context(|| format!("failed to extract {} dictionary zip archive", name))?;

    let extracted = version_dir.join(format!("{}-{}", name, version));
    if extracted.is_dir() {
        fs::rename(&extracted, &dict_dir).with_context(|| {
            format!(
                "failed to rename {} to {}",
                extracted.display(),
                dict_dir.display()
            )
        })?;
    }

    if !dict_dir.is_dir() {
        bail!(
            "dictionary directory not found after extraction: {}",
            dict_dir.display()
        );
    }

    Ok(dict_dir)
}

/// Lists all installed dictionaries, sorted by version and name.
pub fn list_dictionaries(base: Option<&Path>) -> Result<Vec<DictEntry>> {
    let root = dictionaries_root_or(base)?;
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
        for dict_entry in fs::read_dir(version_entry.path()).with_context(|| {
            format!(
                "failed to read directory {}",
                version_entry.path().display()
            )
        })? {
            let dict_entry = dict_entry?;
            if !dict_entry.file_type()?.is_dir() {
                continue;
            }
            entries.push(DictEntry {
                name: dict_entry.file_name().to_string_lossy().into_owned(),
                version: version.clone(),
                path: dict_entry.path(),
            });
        }
    }

    entries.sort_by(|a, b| (&a.version, &a.name).cmp(&(&b.version, &b.name)));
    Ok(entries)
}

/// Deletes a dictionary by name and version.
pub fn delete_dictionary(name: &str, version: &str, base: Option<&Path>) -> Result<()> {
    validate_segment(name)?;
    validate_segment(version)?;
    let dict_dir = dictionaries_root_or(base)?.join(version).join(name);
    if !dict_dir.is_dir() {
        bail!("dictionary not found: {}", dict_dir.display());
    }
    fs::remove_dir_all(&dict_dir)
        .with_context(|| format!("failed to delete {}", dict_dir.display()))?;
    Ok(())
}

/// Removes dictionary directories for versions other than the current `lindera` version.
///
/// Returns the list of removed version strings.
pub fn clean_old_versions(base: Option<&Path>) -> Result<Vec<String>> {
    let root = dictionaries_root_or(base)?;
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let current_version = lindera::get_version();
    let mut removed = Vec::new();

    for entry in fs::read_dir(&root)
        .with_context(|| format!("failed to read directory {}", root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let version = entry.file_name().to_string_lossy().into_owned();
        if version == current_version {
            continue;
        }
        fs::remove_dir_all(entry.path())
            .with_context(|| format!("failed to delete {}", entry.path().display()))?;
        removed.push(version);
    }

    Ok(removed)
}
