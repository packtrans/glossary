use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use packtrans_glossary_core::schema::build_schema;
use packtrans_glossary_core::{text_component, tokenizer, util};
use serde_json::Value;
use tantivy::{Index, IndexSettings, TantivyDocument, directory::MmapDirectory};

/// Options for building a search index.
pub struct IndexOptions {
    /// Directory containing mod folders with language files.
    pub scan_dir: PathBuf,
    /// Target language code (used to locate tokenizer and output sub-directory).
    pub lang: String,
    /// Path to the Tantivy index directory to create.
    pub out: PathBuf,
    /// Custom base path for dictionary lookup.
    pub dict_path: Option<PathBuf>,
}

/// Builds a Tantivy index from language files in `scan_dir`.
///
/// Each mod folder is expected to contain `en_us.json` and `{lang}.json`.
pub fn build_index(options: IndexOptions) -> Result<()> {
    util::validate_path_segment(&options.lang, "lang")?;
    if !options.scan_dir.is_dir() {
        bail!(
            "scan dir does not exist or is not a directory: {}",
            options.scan_dir.display()
        );
    }

    let index_dir = &options.out;

    if let Ok(metadata) = index_dir.metadata() {
        if !metadata.is_dir() {
            bail!(
                "index db path already exists and is not a directory: {}",
                index_dir.display()
            );
        }
        if index_dir.read_dir()?.next().is_some() {
            bail!(
                "index db already exists and is not empty: {}",
                index_dir.display()
            );
        }
    }

    if let Some(parent) = index_dir.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create index db parent directory: {}",
                parent.display()
            )
        })?;
    }
    fs::create_dir_all(index_dir).with_context(|| {
        format!(
            "failed to create index db directory: {}",
            index_dir.display()
        )
    })?;

    let (schema, fields) = build_schema(&options.lang);
    let dir = MmapDirectory::open(index_dir)
        .with_context(|| format!("failed to open index directory: {}", index_dir.display()))?;
    let index = Index::create(dir, schema, IndexSettings::default())
        .with_context(|| format!("failed to create index: {}", index_dir.display()))?;

    tokenizer::register_for_language(&index, &options.lang, options.dict_path.as_deref())?;

    let mut writer = index.writer(50_000_000)?;

    let mut total_mods = 0usize;
    let mut lang_file_mods = 0usize;
    let mut indexed_mods = 0usize;
    let mut indexed_docs = 0usize;

    for entry in fs::read_dir(&options.scan_dir)
        .with_context(|| format!("failed to read scan dir: {}", options.scan_dir.display()))?
    {
        let entry = entry?;
        let mod_dir = entry.path();
        if !mod_dir.is_dir() {
            continue;
        }

        total_mods += 1;
        let mod_id = entry.file_name().to_string_lossy().into_owned();
        let source_path = mod_dir.join("en_us.json");
        let target_path = mod_dir.join(format!("{}.json", options.lang));

        if !source_path.is_file() || !target_path.is_file() {
            continue;
        }

        lang_file_mods += 1;
        let source_entries = load_language_file(&source_path);
        let target_entries = load_language_file(&target_path);
        let mut mod_docs = 0usize;

        for (key, source_text) in source_entries {
            let Some(target_text) = target_entries.get(&key) else {
                continue;
            };

            let mut doc = TantivyDocument::default();
            doc.add_text(fields.mod_id, &mod_id);
            doc.add_text(fields.key, &key);
            doc.add_text(fields.source_lang, "en_us");
            doc.add_text(fields.source_text, &source_text);
            doc.add_text(fields.target_lang, &options.lang);
            doc.add_text(fields.target_text, target_text);
            writer.add_document(doc)?;
            indexed_docs += 1;
            mod_docs += 1;
        }

        if mod_docs > 0 {
            indexed_mods += 1;
        }
    }

    writer.commit()?;
    println!(
        "total mods: {total_mods}, lang files: {lang_file_mods}, indexed {indexed_docs} documents from {indexed_mods} mods"
    );

    Ok(())
}

/// Loads a JSON language file into a key-value map.
///
/// Values may be plain strings or Minecraft JSON text components (arrays/objects).
/// Malformed JSON is ignored after printing a warning; an empty map is returned.
fn load_language_file(path: &PathBuf) -> HashMap<String, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!(
                "warning: failed to read language file {}: {err}",
                path.display()
            );
            return HashMap::new();
        }
    };
    const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
    let json = bytes.strip_prefix(&UTF8_BOM).unwrap_or(&bytes);
    let raw: HashMap<String, Value> = match serde_json::from_slice(json) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!(
                "warning: failed to parse language file {}: {err}",
                path.display()
            );
            return HashMap::new();
        }
    };

    let mut entries = HashMap::new();
    for (key, value) in &raw {
        match text_component::flatten_language_value(value, &raw) {
            Some(text) if !text.is_empty() => {
                entries.insert(key.clone(), text);
            }
            Some(_) | None => {}
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_language_file_strips_utf8_bom() {
        let dir = std::env::temp_dir().join(format!(
            "packtrans-glossary-bom-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zh_cn.json");
        const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
        let mut content = Vec::from(UTF8_BOM);
        content.extend_from_slice(r#"{"item.example":"value"}"#.as_bytes());
        fs::write(&path, content).unwrap();

        let map = load_language_file(&path);
        assert_eq!(map.get("item.example"), Some(&"value".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_language_file_flattens_minecraft_text_components() {
        let dir = std::env::temp_dir().join(format!(
            "packtrans-glossary-text-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("en_us.json");
        fs::write(
            &path,
            r#"{
  "item.plain": "Hello",
  "item.things.ender_pouch.tooltip": [
    {"text": "Press ", "color": "gray"},
    {"index": 0, "color": "white"},
    " to open ender Chest inventory"
  ]
}"#,
        )
        .unwrap();

        let map = load_language_file(&path);
        assert_eq!(map.get("item.plain"), Some(&"Hello".to_string()));
        assert_eq!(
            map.get("item.things.ender_pouch.tooltip"),
            Some(&"Press {} to open ender Chest inventory".to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
