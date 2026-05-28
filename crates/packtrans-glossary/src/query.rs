use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use packtrans_glossary_core::dictionary;
use packtrans_glossary_core::schema::fields_from_schema;
use packtrans_glossary_core::{tokenizer, util};
use serde::Serialize;
use tantivy::{
    Index, TantivyDocument,
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{Field, Value},
};

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
pub fn query_index(options: QueryOptions) -> Result<()> {
    let hits = search_index(options)?;
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
    Ok(())
}

/// Queries a Tantivy index and returns matching documents.
pub fn search_index(options: QueryOptions) -> Result<Vec<QueryHit>> {
    util::validate_path_segment(&options.lang, "lang")?;
    let index_dir = indexes::resolve_query_index_dir(&options.lang, options.index_dir.as_deref())?;
    let dir = MmapDirectory::open(&index_dir)
        .with_context(|| format!("failed to open index directory: {}", index_dir.display()))?;
    let index = Index::open(dir)
        .with_context(|| format!("failed to open index: {}", index_dir.display()))?;

    ensure_tokenizer_dictionary(&options.lang, options.dict_path.as_deref())?;
    tokenizer::register_for_language(&index, &options.lang, options.dict_path.as_deref())?;

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

/// Validates HTTP query `limit` (default 10, maximum 50).
pub fn validate_http_limit(limit: Option<usize>) -> Result<usize> {
    const DEFAULT: usize = 10;
    const MAX: usize = 50;
    match limit {
        None => Ok(DEFAULT),
        Some(0) => bail!("limit must be at least 1"),
        Some(n) if n > MAX => bail!("limit must be at most {MAX}"),
        Some(n) => Ok(n),
    }
}

fn ensure_tokenizer_dictionary(lang: &str, base: Option<&std::path::Path>) -> Result<()> {
    let name = tokenizer::target_tokenizer_name(lang);
    if name == "default" {
        return Ok(());
    }

    let expected = dictionary::dictionary_path(name, base)?;
    if expected.is_dir() {
        return Ok(());
    }

    let pb = progress::spinner(format!("Downloading {name} dictionary"));
    let result = dictionary::ensure_dictionary(name, base);
    pb.finish_and_clear();
    result.map(|_| ())
}

/// Retrieves the stored text value for a field from a document.
fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_limit_defaults_and_caps() {
        assert_eq!(validate_http_limit(None).unwrap(), 10);
        assert_eq!(validate_http_limit(Some(1)).unwrap(), 1);
        assert_eq!(validate_http_limit(Some(50)).unwrap(), 50);
        assert!(validate_http_limit(Some(0)).is_err());
        assert!(validate_http_limit(Some(51)).is_err());
    }
}
