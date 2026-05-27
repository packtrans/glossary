use std::path::PathBuf;

use crate::util;
use anyhow::Result;

/// Returns the root directory where search indexes are stored.
pub fn indexes_root() -> Result<PathBuf> {
    Ok(util::data_dir()?.join("packtrans-glossary").join("indexes"))
}
