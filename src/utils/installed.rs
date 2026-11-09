//! Minimal installed-tree reader used by `npm ls` until arborist is ported.

use std::path::Path;

use serde_json::{Value, json};

use crate::npm::Npm;
use crate::utils::errors::Result;

fn read_manifest(dir: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Walks `node_modules` (one level deep plus scoped dirs) from `path`.
pub fn read_tree(_npm: &dyn Npm, path: &Path) -> Result<Value> {
    let manifest = read_manifest(path).unwrap_or_default();
    let mut deps = Vec::new();

    let modules = path.join("node_modules");
    if let Ok(entries) = std::fs::read_dir(&modules) {
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort();

        for name in names {
            let dir = modules.join(&name);
            if name.starts_with('@') {
                if let Ok(scoped) = std::fs::read_dir(&dir) {
                    let mut scoped_names: Vec<String> = scoped
                        .filter_map(Result::ok)
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect();
                    scoped_names.sort();
                    for sub in scoped_names {
                        if let Some(dep) = node_json(&dir.join(&sub), &format!("{name}/{sub}")) {
                            deps.push(dep);
                        }
                    }
                }
            } else if let Some(dep) = node_json(&dir, &name) {
                deps.push(dep);
            }
        }
    }

    let root_name = manifest
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("npmr");
    Ok(json!({
        "name": root_name,
        "version": manifest.get("version"),
        "path": path.to_string_lossy(),
        "dependencies": deps.into_iter().collect::<serde_json::Map<String, Value>>(),
    }))
}

fn node_json(dir: &Path, name: &str) -> Option<(String, Value)> {
    let manifest = read_manifest(dir)?;
    let version = manifest.get("version").cloned().unwrap_or(Value::Null);
    Some((
        name.to_string(),
        json!({
            "version": version,
            "resolved": manifest.get("_resolved").cloned().unwrap_or(Value::Null),
            "path": dir.to_string_lossy(),
        }),
    ))
}

/// Renders the tree as `+-- name@version` lines.
pub fn format_tree(tree: &Value, parseable: bool, long: bool) -> String {
    let mut lines = Vec::new();
    let root_name = tree.get("name").and_then(Value::as_str).unwrap_or("root");
    let root_version = tree.get("version").and_then(Value::as_str).unwrap_or("");
    if parseable {
        lines.push(format!(
            "{} {}",
            tree.get("path").and_then(Value::as_str).unwrap_or(""),
            root_name
        ));
    } else if root_version.is_empty() {
        lines.push(root_name.to_string());
    } else {
        lines.push(format!("{root_name}@{root_version}"));
    }

    if let Some(deps) = tree.get("dependencies").and_then(Value::as_object) {
        let count = deps.len();
        for (i, (name, dep)) in deps.iter().enumerate() {
            let last = i + 1 == count;
            let branch = if last { "└──" } else { "├──" };
            let version = dep.get("version").and_then(Value::as_str).unwrap_or("");
            let mut line = if parseable {
                let path = dep.get("path").and_then(Value::as_str).unwrap_or("");
                format!("{path} {name}@{version}")
            } else if version.is_empty() {
                format!("{branch} {name}")
            } else {
                format!("{branch} {name}@{version}")
            };
            if long {
                if let Some(resolved) = dep.get("resolved").and_then(Value::as_str) {
                    line.push_str(&format!(" {resolved}"));
                }
            }
            lines.push(line);
        }
    }
    lines.join("\n")
}
