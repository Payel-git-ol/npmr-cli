//! Port of `lib/utils/key-values.js`: prints `key: value` blocks for staged
//! package submissions and generic option objects.

use serde_json::{Map, Value};

pub fn default_predicate(_key: &str, value: &Value) -> bool {
    !value.is_null()
}

/// Renders a nested object as aligned `key: value` lines.
pub fn log_object(obj: &Map<String, Value>) -> String {
    log_object_with(obj, default_predicate)
}

pub fn log_object_with(
    obj: &Map<String, Value>,
    predicate: impl Fn(&str, &Value) -> bool,
) -> String {
    let entries: Vec<(&String, &Value)> = obj.iter().filter(|(k, v)| predicate(k, v)).collect();
    let width = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    entries
        .iter()
        .map(|(k, v)| format!("{:width$}: {}", k, display_value(v)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders a single staged item, flattening nested objects one level.
pub fn log_stage_item(item: &Value) -> String {
    match item {
        Value::Object(obj) => log_object(obj),
        other => display_value(other),
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(display_value)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_predicate_skips_null_values() {
        assert!(!default_predicate("k", &Value::Null));
        assert!(default_predicate("k", &Value::String("val".into())));
    }

    #[test]
    fn log_object_aligns_keys_and_skips_null() {
        let obj = serde_json::from_value::<Map<String, Value>>(serde_json::json!({
            "a": "yes",
            "b": null,
            "cc": "also"
        }))
        .unwrap();
        let out = log_object(&obj);
        assert_eq!(out, "a : yes\ncc: also");
    }

    #[test]
    fn log_object_with_custom_predicate() {
        let obj = serde_json::from_value::<Map<String, Value>>(serde_json::json!({
            "a": 1,
            "b": 2
        }))
        .unwrap();
        let out = log_object_with(&obj, |key, _| key == "b");
        assert_eq!(out, "b: 2");
    }

    #[test]
    fn log_stage_item_renders_objects() {
        let item = serde_json::json!({
            "id": "abc",
            "packageName": "pkg",
            "version": "1.0.0",
            "extra": "bonus"
        });
        let out = log_stage_item(&item);
        assert!(out.contains("packageName: pkg"), "{out}");
        assert!(out.contains("extra      : bonus"), "{out}");
    }
}
