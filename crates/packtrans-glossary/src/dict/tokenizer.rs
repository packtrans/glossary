use std::path::Path;

use anyhow::{Context, Result, bail};
use tantivy::Index;

use super::dictionary;
use packtrans_glossary_core::tokenizer::{self, DICTIONARY_NAMES};

pub fn load_dictionary(name: &str, base: Option<&Path>) -> Result<lindera::dictionary::Dictionary> {
    if !DICTIONARY_NAMES.contains(&name) {
        bail!("unknown tokenizer: {}", name);
    }
    let dict_path = dictionary::ensure_dictionary(name, base)?;
    lindera::dictionary::load_fs_dictionary(&dict_path)
        .with_context(|| format!("failed to load {} dictionary", name))
}

pub fn register_for_language(
    index: &Index,
    target_language: &str,
    base: Option<&Path>,
    cached_dict: Option<&lindera::dictionary::Dictionary>,
) -> Result<()> {
    let name = tokenizer::target_tokenizer_name(target_language);
    if name == "default" {
        return Ok(());
    }
    let dict = match cached_dict {
        Some(dict) => dict.clone(),
        None => load_dictionary(name, base)?,
    };
    let segmenter = lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dict, None);
    let tokenizer = lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
    index.tokenizers().register(name, tokenizer);
    Ok(())
}
