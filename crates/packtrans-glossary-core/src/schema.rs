use anyhow::Result;
use tantivy::schema::{Field, STORED, Schema, TEXT};

#[derive(Clone, Copy)]
pub(crate) struct Fields {
    pub mod_id: Field,
    pub key: Field,
    pub source_lang: Field,
    pub source_text: Field,
    pub target_lang: Field,
    pub target_text: Field,
}

pub(crate) fn build_schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let fields = Fields {
        mod_id: builder.add_text_field("mod_id", STORED),
        key: builder.add_text_field("key", STORED),
        source_lang: builder.add_text_field("source_lang", STORED),
        source_text: builder.add_text_field("source_text", TEXT | STORED),
        target_lang: builder.add_text_field("target_lang", STORED),
        target_text: builder.add_text_field("target_text", STORED),
    };

    (builder.build(), fields)
}

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
