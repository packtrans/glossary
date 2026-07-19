//! Lindera dictionary loading aligned with [`lindera-wasm`](https://github.com/lindera/lindera/tree/main/lindera-wasm).
//!
//! Dictionaries are loaded from raw byte arrays (the same format as
//! `loadDictionaryFromBytes` in `lindera-wasm-web`). Zip download and extraction
//! belong on the JavaScript side (for example via `lindera-wasm-web/opfs`).

use anyhow::{Context, Result};
use lindera::dictionary::Dictionary;
use lindera_dictionary::dictionary::character_definition::CharacterDefinition;
use lindera_dictionary::dictionary::connection_cost_matrix::ConnectionCostMatrix;
use lindera_dictionary::dictionary::metadata::Metadata;
use lindera_dictionary::dictionary::prefix_dictionary::PrefixDictionary;
use lindera_dictionary::dictionary::unknown_dictionary::UnknownDictionary;
use wasm_bindgen::prelude::*;

/// A morphological analysis dictionary.
#[wasm_bindgen(js_name = "Dictionary")]
#[derive(Clone)]
pub struct JsDictionary {
    pub(crate) inner: Dictionary,
}

#[wasm_bindgen(js_class = "Dictionary")]
impl JsDictionary {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.metadata.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn encoding(&self) -> String {
        self.inner.metadata.encoding.clone()
    }
}

/// Loads a dictionary from raw byte arrays.
///
/// This matches `loadDictionaryFromBytes` in `lindera-wasm-web`.
#[wasm_bindgen(js_name = "loadDictionaryFromBytes")]
#[allow(clippy::too_many_arguments)]
pub fn load_dictionary_from_bytes(
    metadata: &[u8],
    dict_da: &[u8],
    dict_vals: &[u8],
    dict_words_idx: &[u8],
    dict_words: &[u8],
    matrix_mtx: &[u8],
    char_def: &[u8],
    unk: &[u8],
) -> Result<JsDictionary, JsValue> {
    load_dictionary_inner(
        metadata,
        dict_da,
        dict_vals,
        dict_words_idx,
        dict_words,
        matrix_mtx,
        char_def,
        unk,
    )
    .map(|inner| JsDictionary { inner })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Snake-case alias matching the `lindera-wasm` Python-style exports.
#[wasm_bindgen(js_name = "load_dictionary_from_bytes")]
#[allow(clippy::too_many_arguments)]
pub fn load_dictionary_from_bytes_snake(
    metadata: &[u8],
    dict_da: &[u8],
    dict_vals: &[u8],
    dict_words_idx: &[u8],
    dict_words: &[u8],
    matrix_mtx: &[u8],
    char_def: &[u8],
    unk: &[u8],
) -> Result<JsDictionary, JsValue> {
    load_dictionary_from_bytes(
        metadata,
        dict_da,
        dict_vals,
        dict_words_idx,
        dict_words,
        matrix_mtx,
        char_def,
        unk,
    )
}

#[allow(clippy::too_many_arguments)]
fn load_dictionary_inner(
    metadata: &[u8],
    dict_da: &[u8],
    dict_vals: &[u8],
    dict_words_idx: &[u8],
    dict_words: &[u8],
    matrix_mtx: &[u8],
    char_def: &[u8],
    unk: &[u8],
) -> Result<Dictionary> {
    let meta = Metadata::load(metadata).context("metadata")?;

    let prefix_dictionary = PrefixDictionary::load(
        dict_da.to_vec(),
        dict_vals.to_vec(),
        dict_words_idx.to_vec(),
        dict_words.to_vec(),
        true,
    )
    .context("prefix_dict")?;
    let connection_cost_matrix =
        ConnectionCostMatrix::load(matrix_mtx.to_vec()).context("connection")?;
    let character_definition = CharacterDefinition::load(char_def).context("char_def")?;
    let unknown_dictionary = UnknownDictionary::load(unk).context("unk")?;

    Ok(Dictionary {
        prefix_dictionary,
        connection_cost_matrix,
        character_definition,
        unknown_dictionary,
        metadata: meta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_metadata() {
        let result = load_dictionary_inner(b"not valid json", &[], &[], &[], &[], &[], &[], &[]);
        let err = match result {
            Err(err) => err.to_string(),
            Ok(_) => panic!("expected metadata parse to fail"),
        };

        assert!(
            err.contains("metadata"),
            "error should mention metadata: {err}"
        );
    }

    #[test]
    fn rejects_incomplete_metadata() {
        let metadata = br#"{"name":"test","encoding":"utf-8"}"#;
        let result = load_dictionary_inner(metadata, &[], &[], &[], &[], &[], &[], &[]);
        assert!(result.is_err());
    }
}
