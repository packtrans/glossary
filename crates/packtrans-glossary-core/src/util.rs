use std::path::PathBuf;

use anyhow::{Context, Result};

/// Returns the platform-specific user data directory.
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
