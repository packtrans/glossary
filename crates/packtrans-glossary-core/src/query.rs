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
    pub inverse: bool,
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

    register_all_tokenizers(&index);

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

    let (src_field, tgt_field) = if options.inverse {
        (fields.target_text, fields.source_text)
    } else {
        (fields.source_text, fields.target_text)
    };

    println!("confidence\tmod_id\tkey\tsource\ttarget_lang\ttarget");
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher.doc(address)?;
        println!(
            "{score:.2}\t{}\t{}\t{}\t{}\t{}",
            stored_text(&doc, fields.mod_id),
            stored_text(&doc, fields.key),
            stored_text(&doc, src_field),
            stored_text(&doc, fields.target_lang),
            stored_text(&doc, tgt_field)
        );
    }

    Ok(())
}

fn register_all_tokenizers(index: &Index) {
    index
        .tokenizers()
        .register("jieba", tantivy_jieba::JiebaTokenizer::new());

    if let Ok(dictionary) = lindera::dictionary::load_dictionary("embedded://ipadic") {
        let segmenter =
            lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dictionary, None);
        let tokenizer = lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
        index.tokenizers().register("lindera", tokenizer);
    }

    if let Ok(dictionary) = lindera::dictionary::load_dictionary("embedded://ko-dic") {
        let segmenter =
            lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dictionary, None);
        let tokenizer = lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
        index.tokenizers().register("lindera_ko", tokenizer);
    }
}

fn stored_text(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}
