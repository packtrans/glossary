use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

#[cfg(target_arch = "wasm32")]
mod mem;

pub fn load_dictionary_from_zip(zip_bytes: &[u8]) -> Result<lindera::dictionary::Dictionary> {
    #[cfg(target_arch = "wasm32")]
    {
        return mem::load_dictionary_from_zip(zip_bytes);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let temp =
            std::env::temp_dir().join(format!("packtrans-glossary-dict-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp)?;
        }
        crate::archive::materialize_dictionary_zip(zip_bytes, &temp)?;
        lindera::dictionary::load_fs_dictionary(&temp).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

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

/// Returns the root directory where dictionaries are stored.
pub fn dictionaries_root() -> Result<PathBuf> {
    Ok(crate::util::data_dir()?
        .join("packtrans-glossary")
        .join("dictionaries"))
}

/// Resolves the dictionary root, using `base` if provided or falling back to [`dictionaries_root`].
fn dictionaries_root_or(base: Option<&Path>) -> Result<PathBuf> {
    match base {
        Some(p) => Ok(p.to_path_buf()),
        None => dictionaries_root(),
    }
}

/// Returns the expected dictionary directory for a dictionary name under the
/// current `lindera` version (`lindera::get_version()`).
///
/// Validates `name` via [`crate::util::validate_path_segment`] and builds
/// `dictionaries_root_or(base)?.join(version).join(name)`.
pub fn dictionary_path(name: &str, base: Option<&Path>) -> Result<PathBuf> {
    let version = lindera::get_version();
    crate::util::validate_path_segment(name, "dictionary name")?;
    crate::util::validate_path_segment(version, "dictionary version")?;
    Ok(dictionaries_root_or(base)?.join(version).join(name))
}

/// Ensures a dictionary is available locally, downloading it if necessary.
///
/// Returns the path to the dictionary directory.
#[cfg(feature = "native")]
pub fn ensure_dictionary(name: &str, base: Option<&Path>) -> Result<PathBuf> {
    let version = lindera::get_version();
    let root = dictionaries_root_or(base)?;
    let dict_dir = dictionary_path(name, base)?;
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
    crate::util::validate_path_segment(name, "dictionary name")?;
    crate::util::validate_path_segment(version, "dictionary version")?;
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
