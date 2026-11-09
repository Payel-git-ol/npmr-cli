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
