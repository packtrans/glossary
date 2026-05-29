use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use tantivy::Index;
use tantivy::directory::{Directory, RamDirectory, TerminatingWrite, WritePtr};
use zip::ZipArchive;

pub fn extract_zip_to_map(zip_bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader).context("failed to parse zip archive")?;
    let mut files = HashMap::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let rel_path = enclosed_name.to_string_lossy().replace('\\', "/");
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .with_context(|| format!("failed to read zip entry {rel_path}"))?;
        files.insert(rel_path, data);
    }

    if files.is_empty() {
        bail!("zip archive contained no files");
    }
    Ok(files)
}

pub fn open_index_from_zip(zip_bytes: &[u8], lang: &str) -> Result<Index> {
    let files = extract_zip_to_map(zip_bytes)?;
    let prefix = find_lang_prefix(&files, lang)?;
    let ram = ram_directory_from_prefix(&files, &prefix)?;
    Index::open(ram).with_context(|| format!("failed to open index for {lang}"))
}

fn find_lang_prefix(files: &HashMap<String, Vec<u8>>, lang: &str) -> Result<String> {
    let direct = format!("{lang}/");
    if files.keys().any(|path| path.starts_with(&direct)) {
        return Ok(direct);
    }

    for path in files.keys() {
        if path.ends_with(&format!("{lang}/meta.json")) {
            let prefix = path
                .strip_suffix(&format!("{lang}/meta.json"))
                .unwrap_or(path);
            return Ok(format!("{prefix}{lang}/"));
        }
    }

    bail!("zip archive did not contain expected {lang} index directory");
}

fn ram_directory_from_prefix(
    files: &HashMap<String, Vec<u8>>,
    prefix: &str,
) -> Result<RamDirectory> {
    let ram = RamDirectory::create();
    for (rel_path, data) in files {
        if !rel_path.starts_with(prefix) {
            continue;
        }
        let index_rel = rel_path.strip_prefix(prefix).unwrap_or(rel_path);
        if index_rel.is_empty() || index_rel.ends_with('/') {
            continue;
        }
        let path = Path::new(index_rel);
        let mut writer: WritePtr = ram
            .open_write(path)
            .with_context(|| format!("failed to open write handle for {index_rel}"))?;
        writer
            .write_all(data)
            .with_context(|| format!("failed to write {index_rel} into ram directory"))?;
        writer
            .terminate()
            .with_context(|| format!("failed to finalize {index_rel}"))?;
    }
    Ok(ram)
}

pub fn dictionary_prefix_from_files(files: &HashMap<String, Vec<u8>>) -> Result<String> {
    for path in files.keys() {
        if path.ends_with("/char_def.bin") {
            let prefix = path.strip_suffix("char_def.bin").unwrap_or(path);
            return Ok(prefix.to_string());
        }
    }
    bail!("dictionary zip did not contain char_def.bin");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn materialize_dictionary_zip(zip_bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use std::fs;

    let files = extract_zip_to_map(zip_bytes)?;
    let prefix = dictionary_prefix_from_files(&files)?;
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)
            .with_context(|| format!("failed to clear {}", dest_dir.display()))?;
    }
    fs::create_dir_all(dest_dir)?;

    for (rel_path, data) in &files {
        if !rel_path.starts_with(&prefix) {
            continue;
        }
        let rel = rel_path.strip_prefix(&prefix).unwrap_or(rel_path.as_str());
        if rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        let out_path = dest_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, data)?;
    }
    Ok(())
}
