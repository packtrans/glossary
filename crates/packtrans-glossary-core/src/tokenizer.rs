use std::path::Path;

use anyhow::{Context, Result, bail};
use tantivy::Index;

use crate::dictionary;

/// Returns the tokenizer name to use for a given target language code.
///
/// - `lzh`, `zh*` → [`dictionary::JIEBA`]
/// - `ja*` → [`dictionary::IPADIC`]
/// - `ko*` → [`dictionary::KO_DIC`]
/// - otherwise → `"default"`
pub fn target_tokenizer_name(target_language: &str) -> &'static str {
    if target_language == "lzh" || target_language.starts_with("zh") {
        dictionary::JIEBA
    } else if target_language.starts_with("ja") {
        dictionary::IPADIC
    } else if target_language.starts_with("ko") {
        dictionary::KO_DIC
    } else {
        "default"
    }
}

/// Loads a named Lindera dictionary from disk, downloading it first when needed.
pub fn load_dictionary(name: &str, base: Option<&Path>) -> Result<lindera::dictionary::Dictionary> {
    if !dictionary::DICTIONARY_NAMES.contains(&name) {
        bail!("unknown tokenizer: {}", name);
    }
    let dict_path = dictionary::ensure_dictionary(name, base)?;
    lindera::dictionary::load_fs_dictionary(&dict_path)
        .with_context(|| format!("failed to load {} dictionary", name))
}

/// Registers the appropriate tokenizer for `target_language` into the given index.
///
/// When `cached_dict` is `Some`, it is used instead of loading from disk.
pub fn register_for_language(
    index: &Index,
    target_language: &str,
    base: Option<&Path>,
    cached_dict: Option<&lindera::dictionary::Dictionary>,
) -> Result<()> {
    let name = target_tokenizer_name(target_language);
    register_by_name(index, name, base, cached_dict)
}

/// Registers a named tokenizer by loading its dictionary from disk.
fn register_by_name(
    index: &Index,
    name: &str,
    base: Option<&Path>,
    cached_dict: Option<&lindera::dictionary::Dictionary>,
) -> Result<()> {
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
