use std::path::PathBuf;

use anyhow::{Context, Result};
use tantivy::{
    Index, TantivyDocument,
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{Field, Value},
};

use crate::schema::fields_from_schema;

pub struct QueryOptions {
    pub query: String,
    pub index_db: PathBuf,
    pub limit: usize,
}

pub fn query_index(options: QueryOptions) -> Result<()> {
    let dir = MmapDirectory::open(&options.index_db).with_context(|| {
        format!(
            "failed to open index directory: {}",
            options.index_db.display()
        )
    })?;
    let index = Index::open(dir)
        .with_context(|| format!("failed to open index: {}", options.index_db.display()))?;
    let schema = index.schema();
    let fields = fields_from_schema(&schema)?;

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![fields.source_text]);
    let parsed_query = query_parser.parse_query(&options.query)?;
    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(options.limit))?;

    println!("confidence\tmod_id\tkey\tsource\ttarget_lang\ttarget");
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher.doc(address)?;
        println!(
            "{score:.2}\t{}\t{}\t{}\t{}\t{}",
            stored_text(&doc, fields.mod_id),
            stored_text(&doc, fields.key),
            stored_text(&doc, fields.source_text),
            stored_text(&doc, fields.target_lang),
            stored_text(&doc, fields.target_text)
        );
    }

    Ok(())
}

fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}
