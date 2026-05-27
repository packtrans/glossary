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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn root() -> &'static Path {
        Path::new("/some/index/root")
    }

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(LOCAL_INDEXES_DIR, "local");
        assert_eq!(DOWNLOADED_INDEXES_DIR, "downloaded");
        assert_eq!(DOWNLOADED_META_FILE, "meta.json");
    }

    #[test]
    fn local_index_dir_produces_correct_path() {
        let path = local_index_dir(root(), "zh_cn").unwrap();
        assert_eq!(path, root().join("local").join("zh_cn"));
    }

    #[test]
    fn local_index_dir_rejects_empty_lang() {
        assert!(local_index_dir(root(), "").is_err());
    }

    #[test]
    fn local_index_dir_rejects_traversal_in_lang() {
        assert!(local_index_dir(root(), "../other").is_err());
    }

    #[test]
    fn downloaded_indexes_root_produces_correct_path() {
        let path = downloaded_indexes_root(root());
        assert_eq!(path, root().join("downloaded"));
    }

    #[test]
    fn downloaded_meta_path_produces_correct_path() {
        let path = downloaded_meta_path(root());
        assert_eq!(path, root().join("downloaded").join("meta.json"));
    }

    #[test]
    fn downloaded_index_dir_produces_correct_path() {
        let path = downloaded_index_dir(root(), "index-20260526", "zh_cn").unwrap();
        assert_eq!(
            path,
            root()
                .join("downloaded")
                .join("index-20260526")
                .join("zh_cn")
        );
    }

    #[test]
    fn downloaded_index_dir_rejects_empty_version() {
        assert!(downloaded_index_dir(root(), "", "zh_cn").is_err());
    }

    #[test]
    fn downloaded_index_dir_rejects_traversal_in_version() {
        assert!(downloaded_index_dir(root(), "../../etc", "zh_cn").is_err());
    }

    #[test]
    fn downloaded_index_dir_rejects_empty_lang() {
        assert!(downloaded_index_dir(root(), "index-20260526", "").is_err());
    }

    #[test]
    fn downloaded_index_dir_rejects_traversal_in_lang() {
        assert!(downloaded_index_dir(root(), "index-20260526", "../sneaky").is_err());
    }
}
