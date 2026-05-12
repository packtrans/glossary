use anyhow::Result;
use tantivy::schema::{Field, IndexRecordOption, Schema, STORED, TextOptions, TEXT};

use crate::tokenizer;

/// Tantivy schema fields used in the glossary index.
#[derive(Clone, Copy)]
pub(crate) struct Fields {
    /// Field for the mod identifier.
    pub mod_id: Field,
    /// Field for the translation key.
    pub key: Field,
    /// Field for the source language code.
    pub source_lang: Field,
    /// Field for the source (English) text.
    pub source_text: Field,
    /// Field for the target language code.
    pub target_lang: Field,
    /// Field for the target language text.
    pub target_text: Field,
}

/// Builds the Tantivy [`Schema`] and resolves its [`Fields`].
///
/// Target text indexing options depend on the language tokenizer.
pub(crate) fn build_schema(target_language: &str) -> (Schema, Fields) {
    let target_text_opts = match tokenizer::target_tokenizer_name(target_language) {
        "default" => TEXT | STORED,
        tokenizer_name => TextOptions::default()
            .set_indexing_options(
                tantivy::schema::TextFieldIndexing::default()
                    .set_tokenizer(tokenizer_name)
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    };

    let mut builder = Schema::builder();
    let fields = Fields {
        mod_id: builder.add_text_field("mod_id", STORED),
        key: builder.add_text_field("key", STORED),
        source_lang: builder.add_text_field("source_lang", STORED),
        source_text: builder.add_text_field("source_text", TEXT | STORED),
        target_lang: builder.add_text_field("target_lang", STORED),
        target_text: builder.add_text_field("target_text", target_text_opts),
    };

    (builder.build(), fields)
}

/// Looks up each field by name from an existing schema.
pub(crate) fn fields_from_schema(schema: &Schema) -> Result<Fields> {
    Ok(Fields {
        mod_id: schema.get_field("mod_id")?,
        key: schema.get_field("key")?,
        source_lang: schema.get_field("source_lang")?,
        source_text: schema.get_field("source_text")?,
        target_lang: schema.get_field("target_lang")?,
        target_text: schema.get_field("target_text")?,
    })
}
