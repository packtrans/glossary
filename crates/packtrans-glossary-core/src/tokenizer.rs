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
pub(crate) fn target_tokenizer_name(target_language: &str) -> &'static str {
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

/// Registers the appropriate tokenizer for `target_language` into the given index.
pub(crate) fn register_for_language(index: &Index, target_language: &str, base: Option<&Path>) -> Result<()> {
    let name = target_tokenizer_name(target_language);
    register_by_name(index, name, base)
}

/// Registers a named tokenizer by loading its dictionary from disk.
fn register_by_name(index: &Index, name: &str, base: Option<&Path>) -> Result<()> {
    if name == "default" {
        return Ok(());
    }
    if !dictionary::DICTIONARY_NAMES.contains(&name) {
        bail!("unknown tokenizer: {}", name);
    }
    let dict_path = dictionary::ensure_dictionary(name, base)?;
    let dict = lindera::dictionary::load_fs_dictionary(&dict_path)
        .with_context(|| format!("failed to load {} dictionary", name))?;
    let segmenter =
        lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dict, None);
    let tokenizer =
        lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
    index.tokenizers().register(name, tokenizer);
    Ok(())
}
