use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use tantivy::{Index, IndexSettings, TantivyDocument, directory::MmapDirectory};

use crate::dictionary;
use crate::schema::build_schema;
use crate::tokenizer;

/// Returns the root directory where search indexes are stored.
pub fn indexes_root() -> Result<PathBuf> {
    Ok(dictionary::data_dir()?
        .join("packtrans-glossary")
        .join("indexes"))
}

/// Validates that `lang` is a non-empty string without path traversal characters.
pub(crate) fn validate_lang(lang: &str) -> Result<()> {
    if lang.is_empty() {
        bail!("lang must not be empty");
    }
    if lang.contains("..") || lang.contains('/') || lang.contains('\\') {
        bail!("lang contains invalid characters: {}", lang);
    }
    Ok(())
}

/// Options for building a search index.
pub struct IndexOptions {
    /// Directory containing mod folders with language files.
    pub scan_dir: PathBuf,
    /// Target language code (used to locate tokenizer and output sub-directory).
    pub lang: String,
    /// Custom path for the index output. Uses [`indexes_root`] if `None`.
    pub index_path: Option<PathBuf>,
    /// Custom base path for dictionary lookup.
    pub dict_path: Option<PathBuf>,
}

/// Builds a Tantivy index from language files in `scan_dir`.
///
/// Each mod folder is expected to contain `en_us.json` and `{lang}.json`.
pub fn build_index(options: IndexOptions) -> Result<()> {
    validate_lang(&options.lang)?;
    let index_path = match options.index_path {
        Some(path) => path,
        None => indexes_root()?,
    };
    if !options.scan_dir.is_dir() {
        bail!(
            "scan dir does not exist or is not a directory: {}",
            options.scan_dir.display()
        );
    }

    let index_dir = index_path.join(&options.lang);

    if let Ok(metadata) = index_dir.metadata() {
        if !metadata.is_dir() {
            bail!(
                "index db already exists and is not empty: {}",
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
    fs::create_dir_all(&index_dir).with_context(|| {
        format!(
            "failed to create index db directory: {}",
            index_dir.display()
        )
    })?;

    let (schema, fields) = build_schema(&options.lang);
    let dir = MmapDirectory::open(&index_dir)
        .with_context(|| format!("failed to open index directory: {}", index_dir.display()))?;
    let index = Index::create(dir, schema, IndexSettings::default())
        .with_context(|| format!("failed to create index: {}", index_dir.display()))?;

    tokenizer::register_for_language(&index, &options.lang, options.dict_path.as_deref())?;

    let mut writer = index.writer(50_000_000)?;

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

        let mod_id = entry.file_name().to_string_lossy().into_owned();
        let source_path = mod_dir.join("en_us.json");
        let target_path = mod_dir.join(format!("{}.json", options.lang));

        if !source_path.is_file() || !target_path.is_file() {
            eprintln!(
                "warning: skipping {mod_id}: missing {} or {}",
                source_path.display(),
                target_path.display()
            );
            continue;
        }

        let source_entries = load_language_file(&source_path)?;
        let target_entries = load_language_file(&target_path)?;
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
    println!("indexed {indexed_docs} documents from {indexed_mods} mods");

    Ok(())
}

/// Loads a JSON language file into a key-value map.
fn load_language_file(path: &PathBuf) -> Result<HashMap<String, String>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open language file: {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse language file: {}", path.display()))
}
