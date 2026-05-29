use std::path::PathBuf;

use anyhow::Result;
use packtrans_glossary_core::query::{QueryHit, QueryOptions as CoreQueryOptions};
use packtrans_glossary_core::{dictionary, tokenizer, util};
use tantivy::Index;
use tantivy::directory::MmapDirectory;

use crate::indexes;
use crate::progress;

/// Options for querying a search index.
pub struct QueryOptions {
    pub query: String,
    pub index_dir: Option<PathBuf>,
    pub lang: String,
    pub limit: usize,
    pub inverse: bool,
    pub dict_path: Option<PathBuf>,
}

/// Queries a Tantivy index and prints matching documents.
pub fn query_index(options: QueryOptions) -> Result<()> {
    util::validate_path_segment(&options.lang, "lang")?;
    let index_dir = indexes::resolve_query_index_dir(&options.lang, options.index_dir.as_deref())?;
    let dir = MmapDirectory::open(&index_dir)
        .map_err(|e| anyhow::anyhow!("failed to open index directory: {index_dir:?}: {e}"))?;
    let index = Index::open(dir)
        .map_err(|e| anyhow::anyhow!("failed to open index: {index_dir:?}: {e}"))?;

    ensure_tokenizer_dictionary(&options.lang, options.dict_path.as_deref())?;

    let hits = packtrans_glossary_core::query::query_index(
        &index,
        CoreQueryOptions {
            query: &options.query,
            lang: &options.lang,
            limit: options.limit,
            inverse: options.inverse,
            dict_zip: None,
            dict_base: options.dict_path.as_deref(),
        },
    )?;

    print_hits(&hits);
    Ok(())
}

fn print_hits(hits: &[QueryHit]) {
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
