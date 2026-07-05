use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use packtrans_glossary_core::dictionary;
use packtrans_glossary_core::schema::fields_from_schema;
use packtrans_glossary_core::{tokenizer, util};
use serde::Serialize;
use tantivy::{
    TantivyDocument,
    collector::TopDocs,
    query::QueryParser,
    schema::{Field, Value},
};

use crate::dict_cache::DictionaryCache;
use crate::download_guard::DownloadCoordinator;
use crate::index_cache::{self, IndexCache};
use crate::indexes;
use crate::progress;

/// Options for querying a search index.
pub struct QueryOptions {
    /// The search query string.
    pub query: String,
    /// Local index root directory; queries `{index_dir}/{lang}`.
    /// When `None`, uses a release download from the default index root.
    pub index_dir: Option<PathBuf>,
    /// Target language code.
    pub lang: String,
    /// Maximum number of results to return.
    pub limit: usize,
    /// If `true`, search target text and output source text.
    pub inverse: bool,
    /// Custom base path for dictionary lookup.
    pub dict_path: Option<PathBuf>,
    /// When set (HTTP server), serializes concurrent downloads for the same resource.
    pub download_guard: Option<Arc<DownloadCoordinator>>,
    /// When set (HTTP server), reuses loaded Lindera dictionaries across requests.
    pub dict_cache: Option<DictionaryCache>,
    /// When set (HTTP server), reuses opened Tantivy indexes across requests.
    pub index_cache: Option<IndexCache>,
}

/// A single glossary search hit.
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
    util::validate_path_segment(&options.lang, "lang")?;
    let index_dir = indexes::resolve_query_index_dir(
        &options.lang,
        options.index_dir.as_deref(),
        options.download_guard.as_deref(),
    )?;

    ensure_tokenizer_dictionary(
        &options.lang,
        options.dict_path.as_deref(),
        options.download_guard.as_deref(),
    )?;

    let index = match &options.index_cache {
        Some(cache) => cache.get_or_open(
            &index_dir,
            &options.lang,
            options.dict_path.as_deref(),
            options.dict_cache.as_ref(),
        )?,
        None => index_cache::open_index(
            &index_dir,
            &options.lang,
            options.dict_path.as_deref(),
            options.dict_cache.as_ref(),
        )?,
    };

    let schema = index.schema();
    let fields = fields_from_schema(&schema)?;

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let search_field = if options.inverse {
        fields.target_text
    } else {
        fields.source_text
    };
    let query_parser = QueryParser::for_index(&index, vec![search_field]);
    let parsed_query = query_parser.parse_query(&options.query)?;
    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(options.limit))?;

    // Column semantics follow the query direction, not fixed language roles.
    let (out_src_field, out_tgt_field) = if options.inverse {
        (fields.target_text, fields.source_text)
    } else {
        (fields.source_text, fields.target_text)
    };
    let out_src_lang_field = if options.inverse {
        fields.target_lang
    } else {
        fields.source_lang
    };
    let out_tgt_lang_field = if options.inverse {
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

    crate::download_guard::with_download_lock(download_guard, &lock_key, || {
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
