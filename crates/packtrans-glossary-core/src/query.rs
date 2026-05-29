use anyhow::{Context, Result};
use serde::Serialize;
use tantivy::{
    Index, TantivyDocument,
    collector::TopDocs,
    query::QueryParser,
    schema::{Field, Value},
};

use crate::schema::fields_from_schema;
use crate::tokenizer;

#[derive(Debug, Clone, Serialize)]
pub struct QueryHit {
    pub confidence: f32,
    pub mod_id: String,
    pub key: String,
    pub source: String,
    pub source_lang: String,
    pub target_lang: String,
    pub target: String,
}

pub struct QueryOptions<'a> {
    pub query: &'a str,
    pub lang: &'a str,
    pub limit: usize,
    pub inverse: bool,
    pub dict_zip: Option<&'a [u8]>,
    pub dict_base: Option<&'a std::path::Path>,
}

pub fn query_index(index: &Index, options: QueryOptions<'_>) -> Result<Vec<QueryHit>> {
    tokenizer::register_for_language_with_dict_zip(
        index,
        options.lang,
        options.dict_zip,
        options.dict_base,
    )?;

    let schema = index.schema();
    let fields = fields_from_schema(&schema)?;
    let reader = index.reader().context("failed to open index reader")?;
    let searcher = reader.searcher();
    let search_field = if options.inverse {
        fields.target_text
    } else {
        fields.source_text
    };
    let query_parser = QueryParser::for_index(index, vec![search_field]);
    let parsed_query = query_parser
        .parse_query(options.query)
        .context("failed to parse query")?;
    let top_docs = searcher
        .search(&parsed_query, &TopDocs::with_limit(options.limit))
        .context("search failed")?;

    let (out_src_field, out_tgt_field) = if options.inverse {
        (fields.target_text, fields.source_text)
    } else {
        (fields.source_text, fields.target_text)
    };
    let (out_src_lang_field, out_tgt_lang_field) = if options.inverse {
        (fields.target_lang, fields.source_lang)
    } else {
        (fields.source_lang, fields.target_lang)
    };

    let mut hits = Vec::with_capacity(top_docs.len());
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher.doc(address).context("failed to load document")?;
        hits.push(QueryHit {
            confidence: score,
            mod_id: stored_text(&doc, fields.mod_id).to_string(),
            key: stored_text(&doc, fields.key).to_string(),
            source: stored_text(&doc, out_src_field).to_string(),
            source_lang: stored_text(&doc, out_src_lang_field).to_string(),
            target_lang: stored_text(&doc, out_tgt_lang_field).to_string(),
            target: stored_text(&doc, out_tgt_field).to_string(),
        });
    }
    Ok(hits)
}

fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}
