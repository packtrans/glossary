use anyhow::Result;
use lindera::dictionary::Dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use tantivy::Index;

use crate::lindera_tantivy::LinderaTokenizer;

/// Returns the tokenizer name to use for a given target language code.
///
/// Mirrors `packtrans-glossary-core::tokenizer::target_tokenizer_name`.
pub fn target_tokenizer_name(target_language: &str) -> &'static str {
    if target_language == "lzh" || target_language.starts_with("zh") {
        "lindera-jieba"
    } else if target_language.starts_with("ja") {
        "lindera-ipadic"
    } else if target_language.starts_with("ko") {
        "lindera-ko-dic"
    } else {
        "default"
    }
}

/// Registers a Lindera tokenizer on `index` when `lang` requires one.
pub fn register_tokenizer(index: &Index, lang: &str, dict: &Dictionary) -> Result<()> {
    let name = target_tokenizer_name(lang);
    if name == "default" {
        return Ok(());
    }

    let segmenter = Segmenter::new(Mode::Normal, dict.clone(), None);
    let tokenizer = LinderaTokenizer::from_segmenter(segmenter);
    index.tokenizers().register(name, tokenizer);
    Ok(())
}
