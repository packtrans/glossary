use std::path::PathBuf;

use anyhow::{Context, Result};
use tantivy::{
    Index, TantivyDocument,
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{Field, Value},
};

use crate::index::{indexes_root, validate_lang};
use crate::schema::fields_from_schema;
use crate::tokenizer;

/// Options for querying a search index.
pub struct QueryOptions {
    /// The search query string.
    pub query: String,
    /// Custom path to the index. Uses [`indexes_root`] if `None`.
    pub index_path: Option<PathBuf>,
    /// Target language code.
    pub lang: String,
    /// Maximum number of results to return.
    pub limit: usize,
    /// If `true`, search target text and output source text.
    pub inverse: bool,
    /// Custom base path for dictionary lookup.
    pub dict_path: Option<PathBuf>,
}

/// Queries a Tantivy index and prints matching documents.
pub fn query_index(options: QueryOptions) -> Result<()> {
    validate_lang(&options.lang)?;
    let index_path = match options.index_path {
        Some(path) => path,
        None => indexes_root()?,
    };
    let index_dir = index_path.join(&options.lang);
    let dir = MmapDirectory::open(&index_dir)
        .with_context(|| format!("failed to open index directory: {}", index_dir.display()))?;
    let index = Index::open(dir)
        .with_context(|| format!("failed to open index: {}", index_dir.display()))?;

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

    let (out_src_field, out_tgt_field) = if options.inverse {
        (fields.target_text, fields.source_text)
    } else {
        (fields.source_text, fields.target_text)
    };
    let out_lang_field = if options.inverse {
        fields.source_lang
    } else {
        fields.target_lang
    };

    println!("confidence\tmod_id\tkey\tsource\ttarget_lang\ttarget");
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher.doc(address)?;
        println!(
            "{score:.2}\t{}\t{}\t{}\t{}\t{}",
            stored_text(&doc, fields.mod_id),
            stored_text(&doc, fields.key),
            stored_text(&doc, out_src_field),
            stored_text(&doc, out_lang_field),
            stored_text(&doc, out_tgt_field)
        );
    }

    Ok(())
}

/// Retrieves the stored text value for a field from a document.
fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}
