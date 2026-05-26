//! Flattens [Minecraft Java Edition text components] to plain strings for indexing.
//!
//! [Minecraft Java Edition text components]: https://minecraft.wiki/w/Text_component_format

use std::collections::HashMap;

use serde_json::{Map, Value};

/// Flattens a language-file value (plain string or text component) to searchable text.
pub fn flatten_language_value(
    value: &Value,
    translations: &HashMap<String, Value>,
) -> Option<String> {
    flatten_component(value, translations)
}

/// A component can be a string, list, or object (wiki § Java Edition).
fn flatten_component(value: &Value, translations: &HashMap<String, Value>) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text: String = parts
                .iter()
                .filter_map(|part| flatten_component(part, translations))
                .collect();
            Some(text)
        }
        Value::Object(obj) => flatten_object_component(obj, translations),
        _ => None,
    }
}

fn flatten_object_component(
    obj: &Map<String, Value>,
    translations: &HashMap<String, Value>,
) -> Option<String> {
    let mut out = flatten_object_content(obj, translations)?;
    if let Some(Value::Array(extra)) = obj.get("extra") {
        for part in extra {
            if let Some(text) = flatten_component(part, translations) {
                out.push_str(&text);
            }
        }
    }
    Some(out)
}

/// Resolves the content of an object component (before `extra` siblings).
fn flatten_object_content(
    obj: &Map<String, Value>,
    translations: &HashMap<String, Value>,
) -> Option<String> {
    match detect_content_type(obj) {
        ContentType::Text => Some(
            obj.get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
        ContentType::Translate => flatten_translate(obj, translations),
        ContentType::Score => Some(String::new()),
        ContentType::Selector => obj
            .get("selector")
            .and_then(Value::as_str)
            .map(str::to_string),
        ContentType::Keybind => obj.get("keybind").and_then(Value::as_str).map(|id| {
            // Actual key label is client-specific; keep the binding id for search.
            format!("{{keybind:{id}}}")
        }),
        ContentType::Nbt => Some(String::new()),
        ContentType::Object => flatten_object_sprite(obj),
        ContentType::Unknown => flatten_legacy_index(obj),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentType {
    Text,
    Translate,
    Score,
    Selector,
    Keybind,
    Nbt,
    Object,
    Unknown,
}

/// Content type from explicit `type` or implicit tag order (wiki § Content).
fn detect_content_type(obj: &Map<String, Value>) -> ContentType {
    if let Some(explicit) = obj.get("type").and_then(Value::as_str)
        && let Some(content_type) = content_type_from_name(explicit)
    {
        return content_type;
    }
    if obj.contains_key("text") {
        return ContentType::Text;
    }
    if obj.contains_key("translate") {
        return ContentType::Translate;
    }
    if obj.contains_key("score") {
        return ContentType::Score;
    }
    if obj.contains_key("selector") {
        return ContentType::Selector;
    }
    if obj.contains_key("keybind") {
        return ContentType::Keybind;
    }
    if obj.contains_key("nbt") {
        return ContentType::Nbt;
    }
    if obj.contains_key("object") || obj.contains_key("sprite") || obj.contains_key("player") {
        return ContentType::Object;
    }
    ContentType::Unknown
}

fn content_type_from_name(name: &str) -> Option<ContentType> {
    match name {
        "text" => Some(ContentType::Text),
        "translatable" => Some(ContentType::Translate),
        "score" => Some(ContentType::Score),
        "selector" => Some(ContentType::Selector),
        "keybind" => Some(ContentType::Keybind),
        "nbt" => Some(ContentType::Nbt),
        "object" => Some(ContentType::Object),
        _ => None,
    }
}

fn flatten_translate(
    obj: &Map<String, Value>,
    translations: &HashMap<String, Value>,
) -> Option<String> {
    let key = obj.get("translate")?.as_str()?;
    let format = lookup_translation(key, translations)
        .or_else(|| {
            obj.get("fallback")
                .and_then(Value::as_str)
                .and_then(|fallback| lookup_translation(fallback, translations))
        })
        .unwrap_or_else(|| key.to_string());

    let with = obj
        .get("with")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    Some(apply_with_slots(&format, with, translations))
}

fn lookup_translation(key: &str, translations: &HashMap<String, Value>) -> Option<String> {
    let value = translations.get(key)?;
    match value {
        Value::String(text) => Some(text.clone()),
        other => flatten_component(other, translations),
    }
}

/// Substitutes `%s` and `%N$s` slots using flattened `with` arguments (wiki § Translated Text).
fn apply_with_slots(format: &str, with: &[Value], translations: &HashMap<String, Value>) -> String {
    let args: Vec<String> = with
        .iter()
        .filter_map(|arg| flatten_component(arg, translations))
        .collect();

    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();
    let mut next_positional = 0usize;

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('%')) {
            chars.next();
            out.push('%');
            continue;
        }

        let mut index: Option<usize> = None;
        while let Some(&digit) = chars.peek() {
            if !digit.is_ascii_digit() {
                break;
            }
            let digit = digit.to_digit(10).unwrap_or(0) as usize;
            index = Some(index.unwrap_or(0) * 10 + digit);
            chars.next();
        }

        if matches!(chars.peek(), Some('$')) {
            chars.next();
        }

        if chars.next() != Some('s') {
            out.push('%');
            if let Some(idx) = index {
                out.push_str(&idx.to_string());
            }
            continue;
        }

        let arg_index = match index {
            Some(one_based) if one_based > 0 => one_based - 1,
            None => {
                let current = next_positional;
                next_positional += 1;
                current
            }
            _ => continue,
        };

        if let Some(arg) = args.get(arg_index) {
            out.push_str(arg);
        }
    }

    out
}

fn flatten_object_sprite(obj: &Map<String, Value>) -> Option<String> {
    if let Some(Value::String(sprite)) = obj.get("sprite") {
        return Some(format!("[{sprite}]"));
    }
    if obj.get("object").and_then(Value::as_str) == Some("player") {
        return Some("[player]".to_string());
    }
    Some(String::new())
}

/// Some mod language files use `index` for keybind slots (not in the vanilla wiki schema).
fn flatten_legacy_index(obj: &Map<String, Value>) -> Option<String> {
    if obj.contains_key("index") {
        return Some("{}".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map_from(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn string_shorthand() {
        let lang = HashMap::new();
        assert_eq!(
            flatten_component(&json!("hello"), &lang).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn array_concatenates_components() {
        let lang = HashMap::new();
        let value = json!([
            {"text": "Press ", "color": "gray"},
            {"index": 0, "color": "white"},
            " to open"
        ]);
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("Press {} to open")
        );
    }

    #[test]
    fn text_with_extra() {
        let lang = HashMap::new();
        let value = json!({"text": "A", "extra": [{"text": "B"}]});
        assert_eq!(flatten_component(&value, &lang).as_deref(), Some("AB"));
    }

    #[test]
    fn translate_missing_key_uses_key_as_text() {
        let lang = HashMap::new();
        let value = json!({"translate": "gui.done"});
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("gui.done")
        );
    }

    #[test]
    fn translate_substitutes_positional_and_indexed_slots() {
        let lang = map_from(&[("greet", json!("Hello %s and %2$s"))]);
        let value = json!({
            "translate": "greet",
            "with": [{"text": "Alex"}, {"text": "Bob"}]
        });
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("Hello Alex and Bob")
        );
    }

    #[test]
    fn translate_substitutes_plain_with_strings() {
        let lang = HashMap::new();
        let value = json!({
            "translate": "Hello %s",
            "with": ["Steve"]
        });
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("Hello Steve")
        );
    }

    #[test]
    fn translate_uses_fallback_key() {
        let lang = map_from(&[("fallback.key", json!("Fallback text"))]);
        let value = json!({
            "translate": "missing.key",
            "fallback": "fallback.key"
        });
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("Fallback text")
        );
    }

    #[test]
    fn keybind_uses_identifier() {
        let lang = HashMap::new();
        let value = json!({"keybind": "key.inventory"});
        assert_eq!(
            flatten_component(&value, &lang).as_deref(),
            Some("{keybind:key.inventory}")
        );
    }

    #[test]
    fn selector_displays_selector() {
        let lang = HashMap::new();
        let value = json!({"selector": "@p"});
        assert_eq!(flatten_component(&value, &lang).as_deref(), Some("@p"));
    }

    #[test]
    fn score_and_nbt_are_empty() {
        let lang = HashMap::new();
        assert_eq!(
            flatten_component(&json!({"score": {"name": "*", "objective": "obj"}}), &lang)
                .as_deref(),
            Some("")
        );
        assert_eq!(
            flatten_component(&json!({"nbt": "foo", "entity": "@s"}), &lang).as_deref(),
            Some("")
        );
    }

    #[test]
    fn empty_text_object_is_empty() {
        let lang = HashMap::new();
        let value = json!({"italic": false, "text": ""});
        assert_eq!(flatten_component(&value, &lang).as_deref(), Some(""));
    }
}
