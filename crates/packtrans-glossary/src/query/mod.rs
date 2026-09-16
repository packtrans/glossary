use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use packtrans_glossary_core::schema::{Fields, fields_from_schema};
use packtrans_glossary_core::{tokenizer, util};

use crate::dict::dictionary;
use schemars::JsonSchema;
use serde::Serialize;
use tantivy::{
    Index, Searcher, TantivyDocument,
    collector::TopDocs,
    query::{Query, QueryParser, RegexQuery},
    schema::{Field, Value},
};

use crate::dict::DictionaryCache;
use crate::index::{self, IndexCache, open_index};
use crate::util::download_guard::DownloadCoordinator;

use crate::util::progress;

/// Index, language, and limit settings shared by one or more queries.
pub struct QuerySettings {
    /// Local index root directory; queries `{index_dir}/{lang}`.
    /// When `None`, uses a release download from the default index root.
    pub index_dir: Option<PathBuf>,
    /// Target language code.
    pub lang: String,
    /// Maximum number of results to return.
    pub limit: usize,
    /// If `true`, search target text and output source text.
    pub inverse: bool,
    /// If `true`, interpret the query as a regular expression matching indexed terms.
    pub regex: bool,
    /// Custom base path for dictionary lookup.
    pub dict_path: Option<PathBuf>,
    /// When set (HTTP server), serializes concurrent downloads for the same resource.
    pub download_guard: Option<Arc<DownloadCoordinator>>,
    /// When set (HTTP server), reuses loaded Lindera dictionaries across requests.
    pub dict_cache: Option<DictionaryCache>,
    /// When set (HTTP server), reuses opened Tantivy indexes across requests.
    pub index_cache: Option<IndexCache>,
}

/// Options for querying a search index.
pub struct QueryOptions {
    /// The search query string.
    pub query: String,
    /// Index and search settings.
    pub settings: QuerySettings,
}

/// Outcome of one query inside a batch.
///
/// Exactly one of `hits` or `error` is set.
#[derive(Debug, Serialize, JsonSchema)]
pub struct BatchQueryResult {
    /// The query string this result corresponds to.
    pub query: String,
    /// Matching documents; absent when the query failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hits: Option<Vec<QueryHit>>,
    /// Failure message for this query; absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A single glossary search hit.
#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryHit {
    pub confidence: f32,
    pub mod_id: String,
    pub key: String,
    pub source: String,
    pub source_lang: String,
    pub target_lang: String,
    pub target: String,
}

/// Queries a Tantivy index and prints matching documents.
pub fn query_index(options: QueryOptions, json: bool) -> Result<()> {
    let hits = search_index(options)?;
    if json {
        println!("{}", serde_json::to_string(&hits)?);
    } else {
        println!("confidence\tmod_id\tkey\tsource\tsource_lang\ttarget_lang\ttarget");
        for hit in hits {
            println!(
                "{:.2}\t{}\t{}\t{}\t{}\t{}\t{}",
                hit.confidence,
                hit.mod_id,
                hit.key,
                hit.source,
                hit.source_lang,
                hit.target_lang,
                hit.target
            );
        }
    }
    Ok(())
}

/// Queries a Tantivy index and returns matching documents.
pub fn search_index(options: QueryOptions) -> Result<Vec<QueryHit>> {
    let settings = &options.settings;
    util::validate_path_segment(&settings.lang, "lang")?;
    validate_regex_query(
        &settings.lang,
        &options.query,
        settings.inverse,
        settings.regex,
    )?;

    let index = open_query_index(settings)?;
    let fields = fields_from_schema(&index.schema())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    search_one(
        &index,
        &searcher,
        &fields,
        &options.query,
        settings.limit,
        settings.inverse,
        settings.regex,
    )
}

/// Runs the same settings against several query strings, opening the index once.
///
/// The query-independent configuration is validated before the index is
/// touched, so a request that is invalid for every query (e.g. CJK inverse
/// regex) fails without triggering index or dictionary downloads. Per-query
/// input failures (empty query, overlong pattern, unparsable syntax) are
/// reported in the corresponding [`BatchQueryResult`] and do not abort the
/// batch. A per-query failure that is not attributable to its input
/// ([`SearchFailureKind::Internal`], e.g. Tantivy I/O) aborts the whole batch
/// so it keeps the normal error mapping and logging instead of degrading
/// into one quiet row.
pub fn search_index_batch(
    settings: &QuerySettings,
    queries: &[String],
) -> Result<Vec<BatchQueryResult>> {
    util::validate_path_segment(&settings.lang, "lang")?;
    validate_regex_mode(&settings.lang, settings.inverse, settings.regex)?;

    let index = open_query_index(settings)?;
    let fields = fields_from_schema(&index.schema())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut results = Vec::with_capacity(queries.len());
    for query in queries {
        let outcome = if query.is_empty() {
            Err(anyhow::anyhow!("query must not be empty"))
        } else {
            validate_regex_length(query, settings.regex).and_then(|()| {
                search_one(
                    &index,
                    &searcher,
                    &fields,
                    query,
                    settings.limit,
                    settings.inverse,
                    settings.regex,
                )
            })
        };
        match outcome {
            Ok(hits) => results.push(BatchQueryResult {
                query: query.clone(),
                hits: Some(hits),
                error: None,
            }),
            Err(err) => {
                if classify_search_failure(&err) == SearchFailureKind::Internal {
                    return Err(err);
                }
                results.push(BatchQueryResult {
                    query: query.clone(),
                    hits: None,
                    error: Some(err.to_string()),
                });
            }
        }
    }

    Ok(results)
}

/// Resolves the index directory, ensures the tokenizer dictionary is available,
/// and opens the index (reusing the cache when configured).
///
/// Split out of [`search_index`] so batch queries pay this cost once.
fn open_query_index(options: &QuerySettings) -> Result<Index> {
    let index_dir = index::resolve_query_index_dir(
        &options.lang,
        options.index_dir.as_deref(),
        options.download_guard.as_deref(),
    )?;

    ensure_tokenizer_dictionary(
        &options.lang,
        options.dict_path.as_deref(),
        options.download_guard.as_deref(),
    )?;

    match &options.index_cache {
        Some(cache) => cache.get_or_open(
            &index_dir,
            &options.lang,
            options.dict_path.as_deref(),
            options.dict_cache.as_ref(),
        ),
        None => open_index(
            &index_dir,
            &options.lang,
            options.dict_path.as_deref(),
            options.dict_cache.as_ref(),
        ),
    }
}

/// Runs one query against an already-opened index and searcher.
///
/// Callers are responsible for validating `query` with [`validate_regex_query`]
/// and `limit` with [`validate_query_limit`].
fn search_one(
    index: &Index,
    searcher: &Searcher,
    fields: &Fields,
    query: &str,
    limit: usize,
    inverse: bool,
    regex: bool,
) -> Result<Vec<QueryHit>> {
    let search_field = if inverse {
        fields.target_text
    } else {
        fields.source_text
    };
    let parsed_query: Box<dyn Query> = if regex {
        Box::new(
            RegexQuery::from_pattern(query, search_field)
                .with_context(|| format!("failed to parse regex query: {query}"))?,
        )
    } else {
        let query_parser = QueryParser::for_index(index, vec![search_field]);
        query_parser.parse_query(query)?
    };
    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

    // Column semantics follow the query direction, not fixed language roles.
    let (out_src_field, out_tgt_field) = if inverse {
        (fields.target_text, fields.source_text)
    } else {
        (fields.source_text, fields.target_text)
    };
    let out_src_lang_field = if inverse {
        fields.target_lang
    } else {
        fields.source_lang
    };
    let out_tgt_lang_field = if inverse {
        fields.source_lang
    } else {
        fields.target_lang
    };

    let mut hits = Vec::with_capacity(top_docs.len());
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher.doc(address)?;
        hits.push(QueryHit {
            confidence: score,
            mod_id: stored_text(&doc, fields.mod_id).to_owned(),
            key: stored_text(&doc, fields.key).to_owned(),
            source: stored_text(&doc, out_src_field).to_owned(),
            source_lang: stored_text(&doc, out_src_lang_field).to_owned(),
            target_lang: stored_text(&doc, out_tgt_lang_field).to_owned(),
            target: stored_text(&doc, out_tgt_field).to_owned(),
        });
    }

    Ok(hits)
}

fn ensure_tokenizer_dictionary(
    lang: &str,
    base: Option<&std::path::Path>,
    download_guard: Option<&DownloadCoordinator>,
) -> Result<()> {
    let name = tokenizer::target_tokenizer_name(lang);
    if name == "default" {
        return Ok(());
    }

    if dictionary::dictionary_path(name, base)?.is_dir() {
        return Ok(());
    }

    let dict_root = match base {
        Some(path) => path.to_path_buf(),
        None => dictionary::dictionaries_root()?,
    };
    let lock_key = format!("dict:{}:{}", dict_root.display(), name);

    crate::util::download_guard::with_download_lock(download_guard, &lock_key, || {
        if dictionary::dictionary_path(name, base)?.is_dir() {
            return Ok(());
        }
        let pb = progress::spinner(format!("Downloading {name} dictionary"));
        let result = dictionary::ensure_dictionary(name, base);
        pb.finish_and_clear();
        result.map(|_| ())
    })
}

/// Retrieves the stored text value for a field from a document.
fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

/// Maximum pattern length (bytes) accepted for a regex query.
///
/// Caps the cost of compiling the automaton and scanning the term dictionary,
/// preventing unbounded CPU consumption from crafted patterns.
const MAX_REGEX_QUERY_LEN: usize = 64;

/// Validates the query-independent part of a regex search request.
///
/// Batch callers must run this before opening the index so a shared
/// configuration that is invalid for every query (e.g. CJK inverse regex)
/// fails without triggering index or dictionary downloads.
pub(crate) fn validate_regex_mode(lang: &str, inverse: bool, regex: bool) -> Result<()> {
    if regex && inverse && tokenizer::target_tokenizer_name(lang) != "default" {
        bail!("{}", tokenizer::INVERSE_REGEX_CJK_ERROR);
    }
    Ok(())
}

/// Validates the length of a single regex pattern.
fn validate_regex_length(query: &str, regex: bool) -> Result<()> {
    if regex && query.len() > MAX_REGEX_QUERY_LEN {
        bail!(
            "regex query is too long: {} bytes (max {MAX_REGEX_QUERY_LEN} bytes)",
            query.len()
        );
    }
    Ok(())
}

pub(crate) fn validate_regex_query(
    lang: &str,
    query: &str,
    inverse: bool,
    regex: bool,
) -> Result<()> {
    validate_regex_mode(lang, inverse, regex)?;
    validate_regex_length(query, regex)
}

/// Kind of failure returned by [`search_index`] and index resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchFailureKind {
    InvalidInput,
    MissingIndex,
    Internal,
}

/// Classifies search/index resolution failures for MCP tool and HTTP error mapping.
pub(crate) fn classify_search_failure(err: &anyhow::Error) -> SearchFailureKind {
    let msg = err.to_string();
    if msg.starts_with("index directory does not exist:") {
        return SearchFailureKind::MissingIndex;
    }
    if msg.contains("lang contains invalid path component")
        || msg.contains(" must not be empty")
        || msg.contains("failed to parse regex query:")
        || msg.contains("regex query is too long")
        || msg.contains("QueryParserError")
        || msg.contains("regex queries are not supported")
    {
        return SearchFailureKind::InvalidInput;
    }
    SearchFailureKind::Internal
}

/// Validates query `limit` (default 10, maximum 50).
pub(crate) fn validate_query_limit(limit: Option<usize>) -> Result<usize> {
    const DEFAULT: usize = 10;
    const MAX: usize = 50;
    match limit {
        None => Ok(DEFAULT),
        Some(0) => bail!("limit must be at least 1"),
        Some(n) if n > MAX => bail!("limit must be at most {MAX}"),
        Some(n) => Ok(n),
    }
}

/// Maximum number of queries accepted by a single batch request.
pub(crate) const MAX_BATCH_QUERIES: usize = 20;

/// Validates that a batch request carries at least one query within the size cap.
pub(crate) fn validate_batch_size(count: usize) -> Result<()> {
    if count == 0 {
        bail!("`queries` must contain at least one query");
    }
    if count > MAX_BATCH_QUERIES {
        bail!("`queries` must contain at most {MAX_BATCH_QUERIES} queries");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::PathBuf;

    use packtrans_glossary_core::schema::build_schema;
    use tantivy::directory::MmapDirectory;
    use tantivy::{Index, IndexSettings};

    use crate::query::TantivyDocument;

    /// Scratch Tantivy index tree that retries its cleanup on drop.
    ///
    /// Windows keeps mmap-backed files alive briefly after the index, reader,
    /// and writer are dropped; deleting the directory immediately can fail
    /// with `DirectoryNotEmpty`. Retrying keeps the suite stable without
    /// leaking temp directories on persistent failure.
    pub(crate) struct TestIndex {
        root: PathBuf,
    }

    impl TestIndex {
        pub(crate) fn build() -> Self {
            let root = std::env::temp_dir().join(format!(
                "packtrans-glossary-query-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let index_dir = root.join("en_us");
            fs::create_dir_all(&index_dir).unwrap();

            let (schema, fields) = build_schema("en_us");
            let directory = MmapDirectory::open(&index_dir).unwrap();
            let index = Index::create(directory, schema, IndexSettings::default()).unwrap();
            let mut writer = index.writer(15_000_000).unwrap();

            for (key, source, target) in [
                ("cooking_pot", "Cooking Pot", "Stew Pot"),
                ("garden_hoe", "Garden Hoe", "Garden Tool"),
            ] {
                let mut doc = TantivyDocument::default();
                doc.add_text(fields.mod_id, "test");
                doc.add_text(fields.key, key);
                doc.add_text(fields.source_lang, "en_us");
                doc.add_text(fields.source_text, source);
                doc.add_text(fields.target_lang, "en_us");
                doc.add_text(fields.target_text, target);
                writer.add_document(doc).unwrap();
            }
            writer.commit().unwrap();
            drop(writer);
            drop(index);

            Self { root }
        }

        pub(crate) fn path(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for TestIndex {
        fn drop(&mut self) {
            for _ in 0..10 {
                match fs::remove_dir_all(&self.root) {
                    Ok(()) => return,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
            // Panicking here while a test is already unwinding would abort the
            // runner and hide the real failure; report instead in that case.
            if std::thread::panicking() {
                eprintln!(
                    "warning: failed to remove temp test index {}",
                    self.root.display()
                );
            } else {
                panic!("failed to remove temp test index {}", self.root.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TestIndex;
    use super::*;

    fn options(root: &std::path::Path, query: &str, inverse: bool) -> QueryOptions {
        QueryOptions {
            query: query.to_string(),
            settings: QuerySettings {
                index_dir: Some(root.to_path_buf()),
                lang: "en_us".to_string(),
                limit: 10,
                inverse,
                regex: true,
                dict_path: None,
                download_guard: None,
                dict_cache: None,
                index_cache: None,
            },
        }
    }

    #[test]
    fn regex_query_matches_forward_source_terms() {
        let index = TestIndex::build();
        let hits = search_index(options(index.path(), "cook.*", false)).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "cooking_pot");
        assert_eq!(hits[0].source, "Cooking Pot");
        assert_eq!(hits[0].target, "Stew Pot");
    }

    #[test]
    fn regex_query_matches_inverse_target_terms() {
        let index = TestIndex::build();
        let hits = search_index(options(index.path(), "stew.*", true)).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "cooking_pot");
        assert_eq!(hits[0].source, "Stew Pot");
        assert_eq!(hits[0].target, "Cooking Pot");
    }

    #[test]
    fn regex_query_reports_invalid_patterns() {
        let index = TestIndex::build();
        let error = search_index(options(index.path(), "[", false)).unwrap_err();

        assert!(error.to_string().contains("failed to parse regex query"));
    }

    fn batch_settings(root: &std::path::Path) -> QuerySettings {
        QuerySettings {
            index_dir: Some(root.to_path_buf()),
            lang: "en_us".to_string(),
            limit: 10,
            inverse: false,
            regex: true,
            dict_path: None,
            download_guard: None,
            dict_cache: None,
            index_cache: None,
        }
    }

    #[test]
    fn batch_returns_one_result_per_query_in_order() {
        let index = TestIndex::build();
        let queries = vec!["cook.*".to_string(), "garden.*".to_string()];

        let results = search_index_batch(&batch_settings(index.path()), &queries).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].query, "cook.*");
        assert_eq!(results[0].hits.as_ref().unwrap()[0].key, "cooking_pot");
        assert_eq!(results[1].query, "garden.*");
        assert_eq!(results[1].hits.as_ref().unwrap()[0].key, "garden_hoe");
        assert!(results.iter().all(|result| result.error.is_none()));
    }

    #[test]
    fn batch_confines_failures_to_the_failing_query() {
        let index = TestIndex::build();
        let queries = vec!["cook.*".to_string(), "[".to_string(), "".to_string()];

        let results = search_index_batch(&batch_settings(index.path()), &queries).unwrap();

        assert_eq!(results[0].hits.as_ref().unwrap().len(), 1);
        assert!(results[0].error.is_none());

        assert!(results[1].hits.is_none());
        assert!(
            results[1]
                .error
                .as_deref()
                .unwrap()
                .contains("failed to parse regex query")
        );

        assert!(results[2].hits.is_none());
        assert!(results[2].error.as_deref().unwrap().contains("empty"));
    }

    #[test]
    fn batch_rejects_shared_cjk_inverse_regex_before_touching_the_index() {
        let missing = std::env::temp_dir().join(format!(
            "packtrans-glossary-query-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!missing.join("zh_cn").is_dir());
        let settings = QuerySettings {
            index_dir: Some(missing),
            lang: "zh_cn".to_string(),
            limit: 10,
            inverse: true,
            regex: true,
            dict_path: None,
            download_guard: None,
            dict_cache: None,
            index_cache: None,
        };
        let queries = vec!["cook.*".to_string()];

        let error = search_index_batch(&settings, &queries).unwrap_err();

        assert!(
            error
                .to_string()
                .contains(tokenizer::INVERSE_REGEX_CJK_ERROR)
        );
    }

    #[test]
    fn classify_search_failure_kinds() {
        assert_eq!(
            classify_search_failure(&anyhow::anyhow!(
                "index directory does not exist: indexes/zh_cn"
            )),
            SearchFailureKind::MissingIndex
        );
        assert_eq!(
            classify_search_failure(&anyhow::anyhow!(
                "lang contains invalid path component: ../etc"
            )),
            SearchFailureKind::InvalidInput
        );
        assert_eq!(
            classify_search_failure(&anyhow::anyhow!("failed to parse regex query: cook[")),
            SearchFailureKind::InvalidInput
        );
        assert_eq!(
            classify_search_failure(&anyhow::anyhow!("regex query is too long: 65 bytes")),
            SearchFailureKind::InvalidInput
        );
        assert_eq!(
            classify_search_failure(&anyhow::anyhow!("disk read failed")),
            SearchFailureKind::Internal
        );
    }
}

#[cfg(test)]
mod cjk_regex_tests {
    use super::*;

    #[test]
    fn rejects_inverse_regex_queries_for_cjk_languages() {
        let error = search_index(QueryOptions {
            query: ".*".to_string(),
            settings: QuerySettings {
                index_dir: Some(std::env::temp_dir()),
                lang: "zh_cn".to_string(),
                limit: 10,
                inverse: true,
                regex: true,
                dict_path: None,
                download_guard: None,
                dict_cache: None,
                index_cache: None,
            },
        })
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains(tokenizer::INVERSE_REGEX_CJK_ERROR)
        );
    }
}

#[cfg(test)]
mod regex_query_len_tests {
    use super::test_support::TestIndex;
    use super::*;

    #[test]
    fn accepts_regex_query_at_max_length() {
        let query = "a".repeat(MAX_REGEX_QUERY_LEN);
        assert!(validate_regex_query("en_us", &query, false, true).is_ok());
    }

    #[test]
    fn rejects_regex_query_exceeding_max_length() {
        let query = "a".repeat(MAX_REGEX_QUERY_LEN + 1);
        let error = validate_regex_query("en_us", &query, false, true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("regex query is too long"));
        assert!(error.contains(&MAX_REGEX_QUERY_LEN.to_string()));
    }

    #[test]
    fn batch_confines_overlong_regex_to_the_failing_query() {
        let index = TestIndex::build();
        let queries = vec!["cook.*".to_string(), "a".repeat(MAX_REGEX_QUERY_LEN + 1)];

        let results = search_index_batch(
            &QuerySettings {
                index_dir: Some(index.path().to_path_buf()),
                lang: "en_us".to_string(),
                limit: 10,
                inverse: false,
                regex: true,
                dict_path: None,
                download_guard: None,
                dict_cache: None,
                index_cache: None,
            },
            &queries,
        )
        .unwrap();

        assert_eq!(results[0].hits.as_ref().unwrap().len(), 1);
        assert!(results[0].error.is_none());
        assert!(results[1].hits.is_none());
        assert!(
            results[1]
                .error
                .as_deref()
                .unwrap()
                .contains("regex query is too long")
        );
    }

    #[test]
    fn length_cap_does_not_apply_to_plain_queries() {
        let query = "a".repeat(MAX_REGEX_QUERY_LEN + 100);
        assert!(validate_regex_query("en_us", &query, false, false).is_ok());
    }
}
