//! WebAssembly bindings for querying PackTrans glossary Tantivy indexes.
//!
//! This crate never downloads index assets. JavaScript should fetch the release
//! asset or other index archive and pass its bytes into [`GlossaryIndex`].

use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::directory::{Directory, RamDirectory};
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value};
use tantivy::{Index, IndexReader, TantivyDocument};
use wasm_bindgen::prelude::*;
use zip::ZipArchive;

/// A single glossary search hit returned to JavaScript.
#[derive(Debug, Serialize)]
pub struct QueryHit {
    pub confidence: f32,
    pub mod_id: String,
    pub key: String,
    pub source: String,
    pub source_lang: String,
    pub target_lang: String,
    pub target: String,
}

#[derive(Clone, Copy)]
struct Fields {
    mod_id: Field,
    key: Field,
    source_lang: Field,
    source_text: Field,
    target_lang: Field,
    target_text: Field,
}

/// In-memory glossary index backed by caller-provided bytes.
#[wasm_bindgen]
pub struct GlossaryIndex {
    index: Index,
    reader: IndexReader,
    fields: Fields,
    lang: String,
}

#[wasm_bindgen]
impl GlossaryIndex {
    /// Builds an in-memory index from a zip archive.
    ///
    /// The archive may contain Tantivy files at its root or under a `{lang}/`
    /// directory, matching PackTrans release assets.
    #[wasm_bindgen(constructor)]
    pub fn new(index_zip: &[u8], lang: &str) -> std::result::Result<GlossaryIndex, JsValue> {
        Self::from_zip(index_zip, lang).map_err(to_js_error)
    }

    /// Builds an in-memory index from a zip archive.
    #[wasm_bindgen(js_name = fromZipBytes)]
    pub fn from_zip_bytes(
        index_zip: &[u8],
        lang: &str,
    ) -> std::result::Result<GlossaryIndex, JsValue> {
        Self::from_zip(index_zip, lang).map_err(to_js_error)
    }

    /// Queries the in-memory index and returns an array of hits.
    ///
    /// Set `inverse` to `true` to search target text and return target-to-source
    /// hits. Inverse CJK searches require tokenizer support that is not bundled
    /// into this lightweight WASM crate.
    pub fn query(
        &self,
        query: &str,
        limit: usize,
        inverse: bool,
    ) -> std::result::Result<JsValue, JsValue> {
        let hits = self.search(query, limit, inverse).map_err(to_js_error)?;
        serde_wasm_bindgen::to_value(&hits).map_err(to_js_error)
    }
}

impl GlossaryIndex {
    fn from_zip(index_zip: &[u8], lang: &str) -> Result<GlossaryIndex> {
        validate_path_segment(lang, "lang")?;
        let directory = load_zip_into_ram_directory(index_zip, lang)?;
        let index = Index::open(directory).context("failed to open index from zip bytes")?;
        let schema = index.schema();
        let fields = fields_from_schema(&schema)?;
        let reader = index.reader().context("failed to create index reader")?;

        Ok(GlossaryIndex {
            index,
            reader,
            fields,
            lang: lang.to_owned(),
        })
    }

    fn search(&self, query: &str, limit: usize, inverse: bool) -> Result<Vec<QueryHit>> {
        if query.trim().is_empty() {
            bail!("query must not be empty");
        }
        if limit == 0 {
            bail!("limit must be at least 1");
        }
        if inverse && target_tokenizer_name(&self.lang) != "default" {
            bail!(
                "inverse queries for {} require the {} tokenizer, which is not bundled in packtrans-glossary-wasm",
                self.lang,
                target_tokenizer_name(&self.lang)
            );
        }

        let search_field = if inverse {
            self.fields.target_text
        } else {
            self.fields.source_text
        };
        let query_parser = QueryParser::for_index(&self.index, vec![search_field]);
        let parsed_query = query_parser
            .parse_query(query)
            .with_context(|| format!("failed to parse query: {query}"))?;
        let searcher = self.reader.searcher();
        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

        let (out_src_field, out_tgt_field) = if inverse {
            (self.fields.target_text, self.fields.source_text)
        } else {
            (self.fields.source_text, self.fields.target_text)
        };
        let out_src_lang_field = if inverse {
            self.fields.target_lang
        } else {
            self.fields.source_lang
        };
        let out_tgt_lang_field = if inverse {
            self.fields.source_lang
        } else {
            self.fields.target_lang
        };

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, address) in top_docs {
            let doc: TantivyDocument = searcher.doc(address)?;
            let mod_id = stored_text(&doc, self.fields.mod_id);
            let key = stored_text(&doc, self.fields.key);
            let source = stored_text(&doc, out_src_field);
            let source_lang = stored_text(&doc, out_src_lang_field);
            let target_lang = stored_text(&doc, out_tgt_lang_field);
            let target = stored_text(&doc, out_tgt_field);

            if mod_id.is_empty()
                || key.is_empty()
                || source.is_empty()
                || source_lang.is_empty()
                || target_lang.is_empty()
                || target.is_empty()
            {
                continue;
            }

            hits.push(QueryHit {
                confidence: score,
                mod_id: mod_id.to_owned(),
                key: key.to_owned(),
                source: source.to_owned(),
                source_lang: source_lang.to_owned(),
                target_lang: target_lang.to_owned(),
                target: target.to_owned(),
            });
        }

        Ok(hits)
    }
}

/// One-shot query helper for callers that do not need to reuse an index.
#[wasm_bindgen]
pub fn query(
    index_zip: &[u8],
    lang: &str,
    query: &str,
    limit: usize,
    inverse: bool,
) -> std::result::Result<JsValue, JsValue> {
    let index = GlossaryIndex::from_zip(index_zip, lang).map_err(to_js_error)?;
    index.query(query, limit, inverse)
}

fn load_zip_into_ram_directory(index_zip: &[u8], lang: &str) -> Result<RamDirectory> {
    let cursor = Cursor::new(index_zip);
    let mut archive = ZipArchive::new(cursor).context("failed to parse index zip archive")?;
    let directory = RamDirectory::create();
    let mut file_count = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }

        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative_path) = normalize_index_entry_path(&enclosed_name, lang)? else {
            continue;
        };
        if should_skip_index_entry(&relative_path) {
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to read zip entry {}", entry.name()))?;
        directory
            .atomic_write(&relative_path, &bytes)
            .with_context(|| format!("failed to load {}", relative_path.display()))?;
        file_count += 1;
    }

    if file_count == 0 {
        bail!("index zip archive did not contain any readable files");
    }
    Ok(directory)
}

fn should_skip_index_entry(relative_path: &Path) -> bool {
    relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".lock"))
}

fn normalize_index_entry_path(path: &Path, lang: &str) -> Result<Option<PathBuf>> {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return Ok(None);
    };

    let relative_path = match first {
        Component::Normal(value) if value == lang => components.collect::<PathBuf>(),
        Component::Normal(_) => path.to_path_buf(),
        _ => bail!("index zip entry contains invalid path: {}", path.display()),
    };

    if relative_path.as_os_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(relative_path))
}

fn fields_from_schema(schema: &Schema) -> Result<Fields> {
    Ok(Fields {
        mod_id: schema.get_field("mod_id")?,
        key: schema.get_field("key")?,
        source_lang: schema.get_field("source_lang")?,
        source_text: schema.get_field("source_text")?,
        target_lang: schema.get_field("target_lang")?,
        target_text: schema.get_field("target_text")?,
    })
}

fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn target_tokenizer_name(target_language: &str) -> &'static str {
    if target_language == "lzh" || target_language.starts_with("zh") {
        "lindera-jieba"
    } else if target_language.starts_with("ja") {
        "lindera-ipadic"
    } else if target_language.starts_with("ko") {
        "lindera-ko-dic"
    } else {
        "default"
    }
}

fn validate_path_segment(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{kind} must not be empty");
    }
    let mut components = Path::new(value).components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if value.contains('\\') || !is_single_normal_component {
        bail!("{kind} contains invalid path component: {value}");
    }
    Ok(())
}

fn to_js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{self, BufWriter, Write};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use tantivy::schema::{STORED, Schema, TEXT};
    use tantivy::{
        IndexSettings, TantivyDocument,
        directory::{
            AntiCallToken, DirectoryLock, FileHandle, Lock, TerminatingWrite, WatchCallback,
            WatchHandle, WritePtr,
            error::{DeleteError, LockError, OpenReadError, OpenWriteError},
        },
    };
    use zip::write::SimpleFileOptions;

    #[test]
    fn queries_index_zip_with_language_root() {
        let zip_bytes = build_test_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr").unwrap();

        let hits = index.search("Cooking Pot", 10, false).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].mod_id, "farmersdelight");
        assert_eq!(hits[0].key, "block.farmersdelight.cooking_pot");
        assert_eq!(hits[0].source, "Cooking Pot");
        assert_eq!(hits[0].target, "Marmite");
    }

    #[test]
    fn supports_inverse_query_for_default_tokenizer_languages() {
        let zip_bytes = build_test_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr").unwrap();

        let hits = index.search("Marmite", 10, true).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "Marmite");
        assert_eq!(hits[0].source_lang, "fr_fr");
        assert_eq!(hits[0].target, "Cooking Pot");
        assert_eq!(hits[0].target_lang, "en_us");
    }

    #[test]
    fn rejects_inverse_cjk_query_without_bundled_tokenizer() {
        let zip_bytes = build_test_index_zip("zh_cn");
        let index = GlossaryIndex::from_zip(&zip_bytes, "zh_cn").unwrap();

        let err = index.search("厨锅", 10, true).unwrap_err().to_string();

        assert!(err.contains("lindera-jieba tokenizer"));
    }

    #[test]
    fn skips_lock_files_when_loading_index_zip() {
        assert!(should_skip_index_entry(Path::new(".tantivy-meta.lock")));
        assert!(!should_skip_index_entry(Path::new("meta.json")));

        let zip_bytes = build_test_index_zip_with_lock("fr_fr");
        let directory = load_zip_into_ram_directory(&zip_bytes, "fr_fr").unwrap();
        assert!(
            !directory
                .exists(Path::new(".tantivy-meta.lock"))
                .expect("exists check")
        );

        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr").unwrap();
        let hits = index.search("Cooking Pot", 10, false).unwrap();
        assert_eq!(hits.len(), 1);
    }

    fn build_test_index_zip_with_lock(lang: &str) -> Vec<u8> {
        let zip_bytes = build_test_index_zip(lang);
        add_file_to_zip(zip_bytes, &format!("{lang}/.tantivy-meta.lock"), &[])
    }

    fn add_file_to_zip(mut zip_bytes: Vec<u8>, entry_name: &str, data: &[u8]) -> Vec<u8> {
        let cursor = Cursor::new(std::mem::take(&mut zip_bytes));
        let mut archive = ZipArchive::new(cursor).unwrap();
        let output = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(output);

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let existing_name = entry.name().to_string();
            writer
                .start_file(&existing_name, SimpleFileOptions::default())
                .unwrap();
            std::io::copy(&mut entry, &mut writer).unwrap();
        }

        writer
            .start_file(entry_name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn build_test_index_zip(lang: &str) -> Vec<u8> {
        let mut builder = Schema::builder();
        let fields = Fields {
            mod_id: builder.add_text_field("mod_id", STORED),
            key: builder.add_text_field("key", STORED),
            source_lang: builder.add_text_field("source_lang", STORED),
            source_text: builder.add_text_field("source_text", TEXT | STORED),
            target_lang: builder.add_text_field("target_lang", STORED),
            target_text: builder.add_text_field("target_text", TEXT | STORED),
        };
        let schema = builder.build();
        let ram_dir = RamDirectory::create();
        let index = Index::create(ram_dir.clone(), schema, IndexSettings::default()).unwrap();
        let mut writer = index.writer(50_000_000).unwrap();
        let mut doc = TantivyDocument::default();
        doc.add_text(fields.mod_id, "farmersdelight");
        doc.add_text(fields.key, "block.farmersdelight.cooking_pot");
        doc.add_text(fields.source_lang, "en_us");
        doc.add_text(fields.source_text, "Cooking Pot");
        doc.add_text(fields.target_lang, lang);
        doc.add_text(
            fields.target_text,
            if lang == "zh_cn" { "厨锅" } else { "Marmite" },
        );
        writer.add_document(doc).unwrap();
        writer.commit().unwrap();
        drop(writer);

        let recording_dir = RecordingDirectory::default();
        ram_dir.persist(&recording_dir).unwrap();
        zip_index_files(&recording_dir.files(), lang)
    }

    fn zip_index_files(files: &HashMap<PathBuf, Vec<u8>>, lang: &str) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let mut paths = files.keys().collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let name = Path::new(lang)
                .join(path)
                .to_string_lossy()
                .replace('\\', "/");
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&files[path]).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[derive(Clone, Debug, Default)]
    struct RecordingDirectory {
        files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
    }

    impl RecordingDirectory {
        fn files(&self) -> HashMap<PathBuf, Vec<u8>> {
            self.files.lock().unwrap().clone()
        }
    }

    impl Directory for RecordingDirectory {
        fn get_file_handle(
            &self,
            path: &Path,
        ) -> std::result::Result<Arc<dyn FileHandle>, OpenReadError> {
            panic!(
                "unexpected read from recording directory: {}",
                path.display()
            )
        }

        fn delete(&self, path: &Path) -> std::result::Result<(), DeleteError> {
            panic!(
                "unexpected delete from recording directory: {}",
                path.display()
            )
        }

        fn exists(&self, path: &Path) -> std::result::Result<bool, OpenReadError> {
            Ok(self.files.lock().unwrap().contains_key(path))
        }

        fn open_write(&self, path: &Path) -> std::result::Result<WritePtr, OpenWriteError> {
            Ok(BufWriter::new(Box::new(RecordingWriter {
                path: path.to_path_buf(),
                files: Arc::clone(&self.files),
                data: Vec::new(),
            })))
        }

        fn atomic_read(&self, path: &Path) -> std::result::Result<Vec<u8>, OpenReadError> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| OpenReadError::FileDoesNotExist(path.to_path_buf()))
        }

        fn atomic_write(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), data.to_vec());
            Ok(())
        }

        fn sync_directory(&self) -> io::Result<()> {
            Ok(())
        }

        fn acquire_lock(&self, _lock: &Lock) -> std::result::Result<DirectoryLock, LockError> {
            panic!("unexpected lock from recording directory")
        }

        fn watch(&self, _watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
            panic!("unexpected watch from recording directory")
        }
    }

    struct RecordingWriter {
        path: PathBuf,
        files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
        data: Vec<u8>,
    }

    impl Write for RecordingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.files
                .lock()
                .unwrap()
                .insert(self.path.clone(), self.data.clone());
            Ok(())
        }
    }

    impl TerminatingWrite for RecordingWriter {
        fn terminate_ref(&mut self, _: AntiCallToken) -> io::Result<()> {
            self.flush()
        }
    }
}
