use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use lindera::dictionary::Dictionary;
use lindera_dictionary::dictionary::character_definition::CharacterDefinition;
use lindera_dictionary::dictionary::connection_cost_matrix::ConnectionCostMatrix;
use lindera_dictionary::dictionary::metadata::Metadata;
use lindera_dictionary::dictionary::prefix_dictionary::PrefixDictionary;
use lindera_dictionary::dictionary::unknown_dictionary::UnknownDictionary;
use zip::ZipArchive;

/// Loads a Lindera dictionary from a release zip archive in memory.
pub fn load_dictionary_from_zip(bytes: &[u8]) -> Result<Dictionary> {
    let files = extract_dictionary_files(bytes)?;
    load_dictionary_from_files(&files)
}

fn extract_dictionary_files(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("failed to parse dictionary zip archive")?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        if should_skip_entry(&enclosed_name) {
            continue;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .with_context(|| format!("failed to read zip entry {}", entry.name()))?;
        entries.push((enclosed_name, data));
    }

    if entries.is_empty() {
        bail!("dictionary zip archive did not contain any readable files");
    }

    let strip_prefix = common_single_dir_prefix(&entries);
    let mut files = HashMap::new();
    for (path, data) in entries {
        let relative = strip_prefix
            .as_ref()
            .and_then(|prefix| path.strip_prefix(prefix).ok())
            .unwrap_or(&path);
        let Some(file_name) = relative
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        files.insert(file_name.to_owned(), data);
    }

    if !files.contains_key("metadata.json") {
        bail!("dictionary zip did not contain metadata.json");
    }

    Ok(files)
}

fn load_dictionary_from_files(files: &HashMap<String, Vec<u8>>) -> Result<Dictionary> {
    let metadata = load_metadata(files)?;
    let character_definition = load_character_definition(files)?;
    let connection_cost_matrix = load_connection_cost_matrix(files)?;
    let prefix_dictionary = load_prefix_dictionary(files)?;
    let unknown_dictionary = load_unknown_dictionary(files)?;

    Ok(Dictionary {
        prefix_dictionary,
        connection_cost_matrix,
        character_definition,
        unknown_dictionary,
        metadata,
    })
}

fn load_metadata(files: &HashMap<String, Vec<u8>>) -> Result<Metadata> {
    let data = required_file(files, "metadata.json")?;
    map_lindera(Metadata::load(data))
}

fn load_character_definition(files: &HashMap<String, Vec<u8>>) -> Result<CharacterDefinition> {
    let data = required_file(files, "char_def.bin")?;
    map_lindera(CharacterDefinition::load(data))
}

fn load_connection_cost_matrix(files: &HashMap<String, Vec<u8>>) -> Result<ConnectionCostMatrix> {
    let data = required_file(files, "matrix.mtx")?;
    map_lindera(ConnectionCostMatrix::load(data.to_vec()))
}

fn load_prefix_dictionary(files: &HashMap<String, Vec<u8>>) -> Result<PrefixDictionary> {
    let da_data = required_file(files, "dict.da")?;
    let vals_data = required_file(files, "dict.vals")?;
    let words_idx_data = required_file(files, "dict.wordsidx")?;
    let words_data = required_file(files, "dict.words")?;

    map_lindera(PrefixDictionary::load(
        da_data.to_vec(),
        vals_data.to_vec(),
        words_idx_data.to_vec(),
        words_data.to_vec(),
        true,
    ))
}

fn load_unknown_dictionary(files: &HashMap<String, Vec<u8>>) -> Result<UnknownDictionary> {
    let data = required_file(files, "unk.bin")?;
    map_lindera(UnknownDictionary::load(data))
}

fn required_file<'a>(files: &'a HashMap<String, Vec<u8>>, name: &str) -> Result<&'a [u8]> {
    files
        .get(name)
        .map(Vec::as_slice)
        .with_context(|| format!("dictionary zip missing required file: {name}"))
}

fn map_lindera<T>(result: lindera_dictionary::LinderaResult<T>) -> Result<T> {
    result.map_err(|err| anyhow::anyhow!(err.to_string()))
}

fn should_skip_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".lock"))
}

fn common_single_dir_prefix(entries: &[(PathBuf, Vec<u8>)]) -> Option<PathBuf> {
    let mut prefix: Option<PathBuf> = None;
    for (path, _) in entries {
        let mut components = path.components();
        let first = components.next()?;
        if !matches!(first, Component::Normal(_)) {
            return None;
        }
        if components.next().is_some() {
            let candidate = PathBuf::from(first.as_os_str());
            prefix = match &prefix {
                Some(existing) if existing == &candidate => Some(existing.clone()),
                Some(_) => return None,
                None => Some(candidate),
            };
        } else {
            return None;
        }
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_dictionary_files_from_versioned_zip_root() {
        let zip_bytes = build_dictionary_zip(&[(
            "lindera-jieba-4.0.0/metadata.json",
            br#"{"dictionary":"jieba"}"#,
        )]);

        let files = extract_dictionary_files(&zip_bytes).unwrap();

        assert!(files.contains_key("metadata.json"));
    }

    #[test]
    fn rejects_dictionary_zip_without_metadata() {
        let zip_bytes = build_dictionary_zip(&[("dict.da", b"data")]);
        let err = extract_dictionary_files(&zip_bytes)
            .unwrap_err()
            .to_string();
        assert!(err.contains("metadata.json"));
    }

    fn build_dictionary_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}
