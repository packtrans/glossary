pub const IPADIC: &str = "lindera-ipadic";
pub const KO_DIC: &str = "lindera-ko-dic";
pub const JIEBA: &str = "lindera-jieba";
pub const DICTIONARY_NAMES: &[&str] = &[IPADIC, KO_DIC, JIEBA];

pub fn target_tokenizer_name(target_language: &str) -> &'static str {
    if target_language == "lzh" || target_language.starts_with("zh") {
        JIEBA
    } else if target_language.starts_with("ja") {
        IPADIC
    } else if target_language.starts_with("ko") {
        KO_DIC
    } else {
        "default"
    }
}

pub const INVERSE_REGEX_CJK_ERROR: &str = "Regex search cannot be used with inverse mode for Chinese, Japanese, or Korean. Use a plain inverse search, or use regex in forward mode.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cjk_languages_to_dict_names() {
        assert_eq!(target_tokenizer_name("zh_cn"), JIEBA);
        assert_eq!(target_tokenizer_name("lzh"), JIEBA);
        assert_eq!(target_tokenizer_name("ja_jp"), IPADIC);
        assert_eq!(target_tokenizer_name("ko_kr"), KO_DIC);
        assert_eq!(target_tokenizer_name("en_us"), "default");
    }
}
