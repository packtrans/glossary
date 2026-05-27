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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_cn_uses_jieba() {
        assert_eq!(target_tokenizer_name("zh_cn"), dictionary::JIEBA);
    }

    #[test]
    fn zh_tw_uses_jieba() {
        assert_eq!(target_tokenizer_name("zh_tw"), dictionary::JIEBA);
    }

    #[test]
    fn lzh_uses_jieba() {
        assert_eq!(target_tokenizer_name("lzh"), dictionary::JIEBA);
    }

    #[test]
    fn ja_jp_uses_ipadic() {
        assert_eq!(target_tokenizer_name("ja_jp"), dictionary::IPADIC);
    }

    #[test]
    fn ja_uses_ipadic() {
        assert_eq!(target_tokenizer_name("ja"), dictionary::IPADIC);
    }

    #[test]
    fn ko_kr_uses_ko_dic() {
        assert_eq!(target_tokenizer_name("ko_kr"), dictionary::KO_DIC);
    }

    #[test]
    fn ko_uses_ko_dic() {
        assert_eq!(target_tokenizer_name("ko"), dictionary::KO_DIC);
    }

    #[test]
    fn en_us_uses_default() {
        assert_eq!(target_tokenizer_name("en_us"), "default");
    }

    #[test]
    fn de_de_uses_default() {
        assert_eq!(target_tokenizer_name("de_de"), "default");
    }

    #[test]
    fn empty_string_uses_default() {
        assert_eq!(target_tokenizer_name(""), "default");
    }

    #[test]
    fn zh_prefix_variants_all_use_jieba() {
        for lang in &["zh", "zh_cn", "zh_tw", "zh_hk"] {
            assert_eq!(
                target_tokenizer_name(lang),
                dictionary::JIEBA,
                "expected {} to use JIEBA",
                lang
            );
        }
    }
}

/// Registers the appropriate tokenizer for `target_language` into the given index.
pub fn register_for_language(
    index: &Index,
    target_language: &str,
    base: Option<&Path>,
) -> Result<()> {
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
    let segmenter = lindera::segmenter::Segmenter::new(lindera::mode::Mode::Normal, dict, None);
    let tokenizer = lindera_tantivy::tokenizer::LinderaTokenizer::from_segmenter(segmenter);
    index.tokenizers().register(name, tokenizer);
    Ok(())
}
