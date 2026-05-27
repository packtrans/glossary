use std::path::{Path, PathBuf};

use crate::util;
use anyhow::Result;

/// Subdirectory under the index root for locally built indexes.
pub const LOCAL_INDEXES_DIR: &str = "local";

/// Subdirectory under the index root for release-downloaded indexes.
pub const DOWNLOADED_INDEXES_DIR: &str = "downloaded";

/// Metadata file stored under [`DOWNLOADED_INDEXES_DIR`].
pub const DOWNLOADED_META_FILE: &str = "meta.json";

/// Returns the root directory where search indexes are stored.
pub fn indexes_root() -> Result<PathBuf> {
    Ok(util::data_dir()?.join("packtrans-glossary").join("indexes"))
}

/// Returns `index-root/local/{lang}`.
pub fn local_index_dir(index_root: &Path, lang: &str) -> Result<PathBuf> {
    util::validate_path_segment(lang, "lang")?;
    Ok(index_root.join(LOCAL_INDEXES_DIR).join(lang))
}

/// Returns `index-root/downloaded`.
pub fn downloaded_indexes_root(index_root: &Path) -> PathBuf {
    index_root.join(DOWNLOADED_INDEXES_DIR)
}

/// Returns `index-root/downloaded/meta.json`.
pub fn downloaded_meta_path(index_root: &Path) -> PathBuf {
    downloaded_indexes_root(index_root).join(DOWNLOADED_META_FILE)
}

/// Returns `index-root/downloaded/{version}/{lang}`.
pub fn downloaded_index_dir(index_root: &Path, version: &str, lang: &str) -> Result<PathBuf> {
    util::validate_path_segment(version, "release tag")?;
    util::validate_path_segment(lang, "lang")?;
    Ok(downloaded_indexes_root(index_root)
        .join(version)
        .join(lang))
}
