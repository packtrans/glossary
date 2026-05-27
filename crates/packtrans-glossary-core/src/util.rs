use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Maximum bytes allowed when downloading a remote file (mod jars, assets, etc.).
pub const MAX_DOWNLOAD_BYTES: usize = 500 * 1024 * 1024;

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

/// Sanitizes a string so it can be safely used as a path component.
pub fn sanitize_path_part(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches(['-', '.', '_']);
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Validates that `value` is a non-empty path segment without directory traversal.
pub fn validate_path_segment(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{kind} must not be empty");
    }
    if value.contains("..") || value.contains('/') || value.contains('\\') {
        bail!("{kind} contains invalid path component: {value}");
    }
    Ok(())
}

/// Downloads a URL to a local file.
pub fn download_to_file(client: &ureq::Agent, url: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = PathBuf::from(format!("{}.tmp", path.display()));
    let download_result = (|| {
        let response = client
            .get(url)
            .call()
            .with_context(|| format!("failed to download {url}"))?;
        let mut reader = response.into_reader().take((MAX_DOWNLOAD_BYTES as u64) + 1);
        let mut file = File::create(&temp_path)
            .with_context(|| format!("failed to create {}", temp_path.display()))?;
        let copied = io::copy(&mut reader, &mut file)
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        if copied > MAX_DOWNLOAD_BYTES as u64 {
            bail!(
                "download from {url} exceeded the max size of {} bytes",
                MAX_DOWNLOAD_BYTES,
            );
        }
        fs::rename(&temp_path, path).with_context(|| {
            format!(
                "failed to move {} to {}",
                temp_path.display(),
                path.display()
            )
        })
    })();

    if download_result.is_err() && temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }
    download_result
}

/// Extracts the contents of a zip archive to a directory.
pub fn extract_zip_file(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)
            .with_context(|| format!("failed to clear {}", dest_dir.display()))?;
    }
    fs::create_dir_all(dest_dir)?;

    let file =
        File::open(zip_path).with_context(|| format!("failed to open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("failed to read zip archive {}", zip_path.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let output_path = dest_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path)?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&output_path)?;
            io::copy(&mut entry, &mut output)?;
        }
    }

    Ok(())
}

/// Recursively copies the contents of one directory to another.
pub fn copy_dir_contents(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Finds the `assets/*/lang` directory with the most JSON files inside an extracted mod.
pub fn find_best_lang_dir(extracted_dir: &Path) -> Result<PathBuf> {
    fn visit(dir: &Path, best: &mut Option<(PathBuf, usize)>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let path = entry.path();
            if path.file_name().and_then(|v| v.to_str()) == Some("lang")
                && path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.file_name())
                    .and_then(|v| v.to_str())
                    == Some("assets")
            {
                let count = count_json_files(&path)?;
                if count > 0
                    && best
                        .as_ref()
                        .map(|(_, best_count)| count > *best_count)
                        .unwrap_or(true)
                {
                    *best = Some((path.clone(), count));
                }
            }

            visit(&path, best)?;
        }
        Ok(())
    }

    let mut best = None;
    visit(extracted_dir, &mut best)?;
    best.map(|(path, _)| path).ok_or_else(|| {
        anyhow::anyhow!(
            "no assets/*/lang directory found in {}",
            extracted_dir.display()
        )
    })
}

/// Counts JSON files in a directory.
pub fn count_json_files(dir: &Path) -> Result<usize> {
    let mut count = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|v| v.to_str()) == Some("json")
        {
            count += 1;
        }
    }
    Ok(count)
}
