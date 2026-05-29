use std::path::Path;

use anyhow::{Context, Result, bail};
use tantivy::Index;

use crate::dictionary;

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

pub fn register_for_language(
    index: &Index,
    target_language: &str,
    base: Option<&Path>,
) -> Result<()> {
    register_for_language_with_dict_zip(index, target_language, None, base)
}

pub fn register_for_language_with_dict_zip(
    index: &Index,
    target_language: &str,
    dict_zip: Option<&[u8]>,
    base: Option<&Path>,
) -> Result<()> {
    register_by_name(
        index,
        target_tokenizer_name(target_language),
        dict_zip,
        base,
    )
}

fn register_by_name(
    index: &Index,
    name: &str,
    dict_zip: Option<&[u8]>,
    base: Option<&Path>,
) -> Result<()> {
    if name == "default" {
        return Ok(());
    }
    if !dictionary::DICTIONARY_NAMES.contains(&name) {
        bail!("unknown tokenizer: {name}");
    }

    let dict = if let Some(zip_bytes) = dict_zip {
        dictionary::load_dictionary_from_zip(zip_bytes)
            .with_context(|| format!("failed to load {name} dictionary from zip"))?
    } else {
        #[cfg(feature = "native")]
        {
            let dict_path = dictionary::ensure_dictionary(name, base)?;
            lindera::dictionary::load_fs_dictionary(&dict_path)
                .with_context(|| format!("failed to load {name} dictionary"))?
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = base;
            bail!("dictionary zip bytes are required for tokenizer {name}");
        }
    };

    let segmenter = lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dict, None);
    let tokenizer = lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
    index.tokenizers().register(name, tokenizer);
    Ok(())
}
