use std::path::{Path, PathBuf};

use crate::util;
use anyhow::Result;

/// Metadata file stored at the index root.
pub const INDEX_META_FILE: &str = "meta.json";

/// Returns the root directory where release-downloaded indexes are stored.
pub fn indexes_root() -> Result<PathBuf> {
    Ok(util::data_dir()?.join("packtrans-glossary").join("indexes"))
}

/// Returns `index-root/meta.json`.
pub fn index_meta_path(index_root: &Path) -> PathBuf {
    index_root.join(INDEX_META_FILE)
}

/// Returns `base/{lang}` for a local index root passed to `--out` or `--index-dir`.
pub fn lang_index_dir(base: &Path, lang: &str) -> Result<PathBuf> {
    util::validate_path_segment(lang, "lang")?;
    Ok(base.join(lang))
}

/// Returns `index-root/{version}/{lang}`.
pub fn release_index_dir(index_root: &Path, version: &str, lang: &str) -> Result<PathBuf> {
    util::validate_path_segment(version, "release tag")?;
    util::validate_path_segment(lang, "lang")?;
    Ok(index_root.join(version).join(lang))
}
