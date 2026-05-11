use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use tantivy::{Index, IndexSettings, TantivyDocument, directory::MmapDirectory};

use crate::schema::build_schema;

pub struct IndexOptions {
    pub scan_dir: PathBuf,
    pub source: String,
    pub target: String,
    pub index_db: PathBuf,
}

pub fn build_index(options: IndexOptions) -> Result<()> {
    if !options.scan_dir.is_dir() {
        bail!(
            "scan dir does not exist or is not a directory: {}",
            options.scan_dir.display()
        );
    }

    if options.index_db.exists() && options.index_db.read_dir()?.next().is_some() {
        bail!(
            "index db already exists and is not empty: {}",
            options.index_db.display()
        );
    }

    if let Some(parent) = options.index_db.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create index db parent directory: {}",
                parent.display()
            )
        })?;
    }
    fs::create_dir_all(&options.index_db).with_context(|| {
        format!(
            "failed to create index db directory: {}",
            options.index_db.display()
        )
    })?;

    let (schema, fields) = build_schema();
    let dir = MmapDirectory::open(&options.index_db).with_context(|| {
        format!(
            "failed to open index directory: {}",
            options.index_db.display()
        )
    })?;
    let index = Index::create(dir, schema, IndexSettings::default())
        .with_context(|| format!("failed to create index: {}", options.index_db.display()))?;
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
        let source_path = mod_dir.join(format!("{}.json", options.source));
        let target_path = mod_dir.join(format!("{}.json", options.target));

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
            doc.add_text(fields.source_lang, &options.source);
            doc.add_text(fields.source_text, &source_text);
            doc.add_text(fields.target_lang, &options.target);
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

fn load_language_file(path: &PathBuf) -> Result<HashMap<String, String>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open language file: {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse language file: {}", path.display()))
}
