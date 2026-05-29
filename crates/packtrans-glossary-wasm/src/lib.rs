use packtrans_glossary_core::archive::open_index_from_zip;
use packtrans_glossary_core::query::{self, QueryHit, QueryOptions};
use packtrans_glossary_core::tokenizer;
use serde::Serialize;
use tantivy::Index;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct QueryHitJs {
    confidence: f32,
    mod_id: String,
    key: String,
    source: String,
    source_lang: String,
    target_lang: String,
    target: String,
}

impl From<QueryHit> for QueryHitJs {
    fn from(hit: QueryHit) -> Self {
        Self {
            confidence: hit.confidence,
            mod_id: hit.mod_id,
            key: hit.key,
            source: hit.source,
            source_lang: hit.source_lang,
            target_lang: hit.target_lang,
            target: hit.target,
        }
    }
}

#[wasm_bindgen]
pub struct GlossaryEngine {
    index: Index,
    lang: String,
    dict_zip: Option<Vec<u8>>,
}

#[wasm_bindgen]
impl GlossaryEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(
        index_zip: &[u8],
        lang: &str,
        dict_zip: Option<Vec<u8>>,
    ) -> Result<GlossaryEngine, JsValue> {
        let index = open_index_from_zip(index_zip, lang).map_err(js_error)?;
        Ok(Self {
            index,
            lang: lang.to_string(),
            dict_zip,
        })
    }

    pub fn query(&self, text: &str, limit: usize, inverse: bool) -> Result<JsValue, JsValue> {
        let hits = query::query_index(
            &self.index,
            QueryOptions {
                query: text,
                lang: &self.lang,
                limit,
                inverse,
                dict_zip: self.dict_zip.as_deref(),
                dict_base: None,
            },
        )
        .map_err(js_error)?;

        let mapped: Vec<QueryHitJs> = hits.into_iter().map(Into::into).collect();
        serde_wasm_bindgen::to_value(&mapped).map_err(js_error)
    }
}

#[wasm_bindgen]
pub fn target_tokenizer_name(lang: &str) -> String {
    tokenizer::target_tokenizer_name(lang).to_string()
}

#[wasm_bindgen]
pub fn lindera_version() -> String {
    packtrans_glossary_core::dictionary::current_version().to_string()
}

fn js_error(err: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&err.to_string())
}
