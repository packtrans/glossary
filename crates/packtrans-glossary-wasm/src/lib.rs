//! WebAssembly bindings for querying PackTrans glossary Tantivy indexes.
//!
//! This crate never downloads index assets. JavaScript should fetch the release
//! asset or other index archive and pass its bytes into [`GlossaryIndex`].

mod dictionary;
mod lindera_tantivy;
mod tokenizer;

#[cfg(test)]
mod test_fixtures;

use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::directory::{Directory, RamDirectory};
use tantivy::query::{Query, QueryParser, RegexQuery};
use tantivy::schema::{Field, Schema, Value};
use tantivy::{Index, IndexReader, TantivyDocument};
use wasm_bindgen::prelude::*;
use zip::ZipArchive;

/// Lindera release version that dictionary zip archives must match.
#[wasm_bindgen]
pub fn lindera_version() -> String {
    lindera::get_version().to_owned()
}

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
}

#[wasm_bindgen]
impl GlossaryIndex {
    /// Builds an in-memory index from a zip archive.
    ///
    /// The archive may contain Tantivy files at its root or under a `{lang}/`
    /// directory, matching PackTrans release assets.
    ///
    /// Pass optional `dict_zip` bytes to register a Lindera tokenizer for
    /// inverse queries on indexes that use one. When omitted, the default
    /// tokenizer is used.
    #[wasm_bindgen(constructor)]
    pub fn new(
        index_zip: &[u8],
        lang: &str,
        dict_zip: Option<Vec<u8>>,
    ) -> std::result::Result<GlossaryIndex, JsValue> {
        Self::from_zip(index_zip, lang, dict_zip).map_err(to_js_error)
    }

    /// Builds an in-memory index from a zip archive.
    #[wasm_bindgen(js_name = fromZipBytes)]
    pub fn from_zip_bytes(
        index_zip: &[u8],
        lang: &str,
        dict_zip: Option<Vec<u8>>,
    ) -> std::result::Result<GlossaryIndex, JsValue> {
        Self::from_zip(index_zip, lang, dict_zip).map_err(to_js_error)
    }

    /// Queries the in-memory index and returns an array of hits.
    ///
    /// Set `inverse` to `true` to search target text and return target-to-source
    /// hits.
    ///
    /// Set `regex` to `true` to treat `query` as a regular expression matched
    /// against indexed terms in the selected search field.
    pub fn query(
        &self,
        query: &str,
        limit: usize,
        inverse: bool,
        regex: Option<bool>,
    ) -> std::result::Result<JsValue, JsValue> {
        let hits = self
            .search(query, limit, inverse, regex.unwrap_or(false))
            .map_err(to_js_error)?;
        serde_wasm_bindgen::to_value(&hits).map_err(to_js_error)
    }
}

impl GlossaryIndex {
    fn from_zip(index_zip: &[u8], lang: &str, dict_zip: Option<Vec<u8>>) -> Result<GlossaryIndex> {
        validate_path_segment(lang, "lang")?;
        let directory = load_zip_into_ram_directory(index_zip, lang)?;
        let index = Index::open(directory).context("failed to open index from zip bytes")?;
        if let Some(dict_bytes) = dict_zip {
            let dict = dictionary::load_dictionary_from_zip(&dict_bytes)
                .context("failed to load dictionary from zip bytes")?;
            tokenizer::register_tokenizer(&index, lang, &dict)
                .context("failed to register tokenizer from dictionary")?;
        }
        let schema = index.schema();
        let fields = fields_from_schema(&schema)?;
        let reader = index.reader().context("failed to create index reader")?;

        Ok(GlossaryIndex {
            index,
            reader,
            fields,
        })
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        inverse: bool,
        regex: bool,
    ) -> Result<Vec<QueryHit>> {
        if query.trim().is_empty() {
            bail!("query must not be empty");
        }
        if limit == 0 {
            bail!("limit must be at least 1");
        }

        let search_field = if inverse {
            self.fields.target_text
        } else {
            self.fields.source_text
        };
        let parsed_query: Box<dyn Query> = if regex {
            Box::new(
                RegexQuery::from_pattern(query, search_field)
                    .with_context(|| format!("failed to parse regex query: {query}"))?,
            )
        } else {
            let query_parser = QueryParser::for_index(&self.index, vec![search_field]);
            query_parser
                .parse_query(query)
                .with_context(|| format!("failed to parse query: {query}"))?
        };
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
    dict_zip: Option<Vec<u8>>,
    regex: Option<bool>,
) -> std::result::Result<JsValue, JsValue> {
    let index = GlossaryIndex::from_zip(index_zip, lang, dict_zip).map_err(to_js_error)?;
    index.query(query, limit, inverse, regex)
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
    use crate::test_fixtures;

    #[test]
    fn queries_index_zip_with_language_root() {
        let zip_bytes = test_fixtures::build_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();

        let hits = index.search("Cooking Pot", 10, false, false).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].mod_id, "farmersdelight");
        assert_eq!(hits[0].key, "block.farmersdelight.cooking_pot");
        assert_eq!(hits[0].source, "Cooking Pot");
        assert_eq!(hits[0].target, "Marmite");
    }

    #[test]
    fn supports_inverse_query_for_default_tokenizer_languages() {
        let zip_bytes = test_fixtures::build_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();

        let hits = index.search("Marmite", 10, true, false).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "Marmite");
        assert_eq!(hits[0].source_lang, "fr_fr");
        assert_eq!(hits[0].target, "Cooking Pot");
        assert_eq!(hits[0].target_lang, "en_us");
    }

    #[test]
    fn supports_inverse_query_for_zh_cn_without_dictionary() {
        let zip_bytes = test_fixtures::build_index_zip("zh_cn");
        let index = GlossaryIndex::from_zip(&zip_bytes, "zh_cn", None).unwrap();

        let hits = index.search("厨锅", 10, true, false).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "厨锅");
        assert_eq!(hits[0].target, "Cooking Pot");
    }

    #[test]
    fn supports_regex_query_on_source_text() {
        let zip_bytes = test_fixtures::build_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();

        let hits = index.search("cook.*", 10, false, true).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "Cooking Pot");
    }

    #[test]
    fn supports_inverse_regex_query_on_target_text() {
        let zip_bytes = test_fixtures::build_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();

        let hits = index.search("marm.*", 10, true, true).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "Marmite");
        assert_eq!(hits[0].target, "Cooking Pot");
    }

    #[test]
    fn rejects_invalid_regex_pattern() {
        let zip_bytes = test_fixtures::build_index_zip("fr_fr");
        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();

        let err = index
            .search("[", 10, false, true)
            .err()
            .expect("expected invalid regex to fail")
            .to_string();

        assert!(err.contains("regex"));
    }

    #[test]
    fn rejects_invalid_dictionary_zip_at_construction() {
        let zip_bytes = test_fixtures::build_index_zip("zh_cn");
        let err = GlossaryIndex::from_zip(&zip_bytes, "zh_cn", Some(vec![1, 2, 3]))
            .err()
            .expect("expected dictionary load to fail")
            .to_string();

        assert!(err.contains("dictionary"));
    }

    #[test]
    fn exports_lindera_version_matching_dependency() {
        assert_eq!(lindera_version(), lindera::get_version());
        assert!(!lindera_version().is_empty());
    }

    #[test]
    fn skips_lock_files_when_loading_index_zip() {
        assert!(should_skip_index_entry(Path::new(".tantivy-meta.lock")));
        assert!(!should_skip_index_entry(Path::new("meta.json")));

        let zip_bytes = test_fixtures::build_index_zip_with_lock("fr_fr");
        let directory = load_zip_into_ram_directory(&zip_bytes, "fr_fr").unwrap();
        assert!(
            !directory
                .exists(Path::new(".tantivy-meta.lock"))
                .expect("exists check")
        );

        let index = GlossaryIndex::from_zip(&zip_bytes, "fr_fr", None).unwrap();
        let hits = index.search("Cooking Pot", 10, false, false).unwrap();
        assert_eq!(hits.len(), 1);
    }
}
