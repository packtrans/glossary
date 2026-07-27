use anyhow::Result;
use lindera::dictionary::Dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use tantivy::Index;

use crate::lindera_tantivy::LinderaTokenizer;

pub use packtrans_glossary_core::tokenizer::{
    INVERSE_REGEX_CJK_ERROR, target_tokenizer_name,
};

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
