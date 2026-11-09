//! Port of `lib/commands/pkg.js`: `npm pkg get|set|delete|fix`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::commands::arborist_cmd::WorkspaceContext;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("pkg", "Manages your package.json")
    .with_params(&["force", "json", "workspace", "workspaces"])
    .with_usage(&[
        "set <key>=<value> [<key>=<value> ...]",
        "get [<key> [<key> ...]]",
        "delete <key> [<key> ...]",
        "set [<array>[<index>].<key>=<value> ...]",
        "set [<array>[].<key>=<value> ...]",
        "fix",
    ])
    .workspaces();

pub struct Pkg;

impl Pkg {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Pkg {
    fn default() -> Self {
        Self::new()
    }
}

fn load(path: &Path) -> Result<Map<String, Value>> {
    let manifest = path.join("package.json");
    let value = crate::cache::read_json(&manifest).map_err(|err| {
        let message = if std::fs::metadata(&manifest).is_err() {
            format!("Could not read package.json: {err}\n{}", manifest.display())
        } else {
            format!("Failed to parse json\n{err}")
        };
        NpmError::new(message).with_code("EJSONPARSE")
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| NpmError::new("package.json must contain an object").with_code("EJSONPARSE"))
}

fn save(path: &Path, manifest: &Map<String, Value>) -> Result<()> {
    let body = serde_json::to_string_pretty(&Value::Object(manifest.clone())).unwrap_or_default();
    std::fs::write(path.join("package.json"), format!("{body}\n"))?;
    Ok(())
}

/// Splits `a.b[0].c` into path segments; `[]` appends.
enum Seg {
    Key(String),
    Index(usize),
    Append,
}

fn parse_path(key: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut current = String::new();
    let mut chars = key.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !current.is_empty() {
                    segs.push(Seg::Key(std::mem::take(&mut current)));
                }
            }
            '[' => {
                if !current.is_empty() {
                    segs.push(Seg::Key(std::mem::take(&mut current)));
                }
                let mut inner = String::new();
                for n in chars.by_ref() {
                    if n == ']' {
                        break;
                    }
                    inner.push(n);
                }
                if inner.is_empty() {
                    segs.push(Seg::Append);
                } else if let Ok(idx) = inner.parse() {
                    segs.push(Seg::Index(idx));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        segs.push(Seg::Key(current));
    }
    segs
}

fn get_in<'a>(value: &'a Value, segs: &[Seg]) -> Option<&'a Value> {
    let Some((first, rest)) = segs.split_first() else {
        return Some(value);
    };
    match first {
        Seg::Key(key) => value.get(key).and_then(|v| get_in(v, rest)),
        Seg::Index(idx) => value.get(idx).and_then(|v| get_in(v, rest)),
        Seg::Append => None,
    }
}

fn set_in(value: &mut Value, segs: &[Seg], new: Value) {
    let Some((first, rest)) = segs.split_first() else {
        *value = new;
        return;
    };
    match first {
        Seg::Key(key) => {
            if !value.is_object() {
                *value = Value::Object(Map::new());
            }
            if let Value::Object(obj) = value {
                let entry = obj.entry(key.clone()).or_insert_with(|| {
                    if matches!(rest.first(), Some(Seg::Index(_) | Seg::Append)) {
                        Value::Array(Vec::new())
                    } else {
                        Value::Object(Map::new())
                    }
                });
                set_in(entry, rest, new);
            }
        }
        Seg::Index(idx) => {
            if !value.is_array() {
                *value = Value::Array(Vec::new());
            }
            if let Value::Array(arr) = value {
                while arr.len() <= *idx {
                    arr.push(Value::Null);
                }
                set_in(&mut arr[*idx], rest, new);
            }
        }
        Seg::Append => {
            if !value.is_array() {
                *value = Value::Array(Vec::new());
            }
            if let Value::Array(arr) = value {
                let mut item = Value::Null;
                set_in(&mut item, rest, new);
                arr.push(item);
            }
        }
    }
}

fn delete_in(value: &mut Value, segs: &[Seg]) {
    let Some((last, parents)) = segs.split_last() else {
        return;
    };
    let Some(parent) = get_in_mut(value, parents) else {
        return;
    };
    match last {
        Seg::Key(key) => {
            if let Value::Object(obj) = parent {
                obj.remove(key);
            }
        }
        Seg::Index(idx) => {
            if let Value::Array(arr) = parent {
                if *idx < arr.len() {
                    arr.remove(*idx);
                }
            }
        }
        Seg::Append => {}
    }
}

fn get_in_mut<'a>(value: &'a mut Value, segs: &[Seg]) -> Option<&'a mut Value> {
    let Some((first, rest)) = segs.split_first() else {
        return Some(value);
    };
    match first {
        Seg::Key(key) => match value {
            Value::Object(obj) => obj.get_mut(key).and_then(|v| get_in_mut(v, rest)),
            _ => None,
        },
        Seg::Index(idx) => match value {
            Value::Array(arr) => arr.get_mut(*idx).and_then(|v| get_in_mut(v, rest)),
            _ => None,
        },
        Seg::Append => None,
    }
}

fn format_jsonish(value: &Value, _color: bool) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

impl Pkg {
    fn get(
        &self,
        npm: &mut (dyn Npm + '_),
        path: &Path,
        args: &[String],
        workspace: Option<&str>,
    ) -> Result<()> {
        let manifest = load(path)?;
        let json = npm.json() || npm.store().get_bool("json");
        let root = Value::Object(manifest);
        if args.is_empty() {
            if json {
                crate::output::set_meta(true);
                let value = match workspace {
                    Some(ws) => serde_json::json!({ ws: root }),
                    None => root,
                };
                crate::output::standard(value.to_string());
            } else {
                print_entries(npm, &root, workspace);
            }
            return Ok(());
        }

        let mut result = Map::new();
        for arg in args {
            let segs = parse_path(arg);
            let value = get_in(&root, &segs).cloned().unwrap_or(Value::Null);
            result.insert(arg.clone(), value);
        }
        if json {
            crate::output::set_meta(true);
            let value = Value::Object(result);
            let value = match workspace {
                Some(ws) => serde_json::json!({ ws: value }),
                None => value,
            };
            crate::output::standard(value.to_string());
            return Ok(());
        }
        if args.len() == 1 {
            let first = &args[0];
            let value = result.get(first).cloned().unwrap_or(Value::Null);
            let text = match &value {
                Value::String(s) => s.clone(),
                other => format_jsonish(other, npm.color()),
            };
            crate::output::standard(match workspace {
                Some(ws) => format!("{ws} {text}"),
                None => text,
            });
            return Ok(());
        }
        print_entries(npm, &Value::Object(result), workspace);
        Ok(())
    }

    fn set(&self, npm: &dyn Npm, path: &Path, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(META.usage_error("npm pkg set expects a key=value pair of args."));
        }
        let json = npm.json() || npm.store().get_bool("json");
        let mut manifest = Value::Object(load(path)?);
        for arg in args {
            let Some((key, value)) = arg.split_once('=') else {
                return Err(META.usage_error("npm pkg set expects a key=value pair of args."));
            };
            if key.is_empty() || value.is_empty() {
                return Err(META.usage_error("npm pkg set expects a key=value pair of args."));
            }
            let value = if json {
                serde_json::from_str(value).map_err(|err| {
                    NpmError::new(format!("Failed to parse json\n{err}")).with_code("EJSONPARSE")
                })?
            } else {
                Value::String(value.to_string())
            };
            set_in(&mut manifest, &parse_path(key), value);
        }
        save(path, &manifest.as_object().cloned().unwrap_or_default())
    }

    fn delete(&self, path: &Path, args: &[String]) -> Result<()> {
        if args.is_empty() || args.iter().any(String::is_empty) {
            return Err(META.usage_error("npm pkg delete expects key args."));
        }
        let mut manifest = Value::Object(load(path)?);
        for key in args {
            delete_in(&mut manifest, &parse_path(key));
        }
        save(path, &manifest.as_object().cloned().unwrap_or_default())
    }

    /// `npm pkg fix`: strips common mistakes from the manifest.
    fn fix(&self, path: &Path) -> Result<()> {
        let mut manifest = load(path)?;
        if let Some(Value::String(bugs)) = manifest.get("bugs") {
            let bugs = bugs.clone();
            manifest.insert("bugs".into(), serde_json::json!({ "url": bugs }));
        }
        if let Some(Value::String(repo)) = manifest.get("repository") {
            let repo = repo.clone();
            manifest.insert("repository".into(), serde_json::json!({ "url": repo }));
        }
        if let Some(Value::String(name)) = manifest.get("name") {
            let fixed = name.trim_start_matches("./").to_string();
            if &fixed != name {
                manifest.insert("name".into(), Value::String(fixed));
            }
        }
        save(path, &manifest)
    }

    fn exec_at(
        &mut self,
        npm: &mut (dyn Npm + '_),
        args: &[String],
        path: PathBuf,
        workspace: Option<String>,
    ) -> Result<()> {
        if npm.global() {
            return Err(
                NpmError::new("There's no package.json file to manage on global mode")
                    .with_code("EPKGGLOBAL"),
            );
        }
        let Some(cmd) = args.first() else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        match cmd.as_str() {
            "get" => self.get(npm, &path, rest, workspace.as_deref()),
            "set" => self.set(npm, &path, rest),
            "delete" => self.delete(&path, rest),
            "fix" => self.fix(&path),
            _ => Err(META.usage_error("")),
        }
    }
}

fn print_entries(npm: &dyn Npm, value: &Value, workspace: Option<&str>) {
    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let rendered = match val {
                Value::String(s) => format!("{s:?}"),
                other => format_jsonish(other, npm.color()),
            };
            crate::output::standard(match workspace {
                Some(ws) => format!("{ws} {key} = {rendered}"),
                None => format!("{key} = {rendered}"),
            });
        }
    }
}

impl Command for Pkg {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let path = npm.local_prefix().to_path_buf();
        self.exec_at(npm, args, path, None)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let mut ctx = WorkspaceContext::new(npm, workspaces.clone());
        for (name, path) in workspaces {
            self.exec_at(&mut ctx, args, PathBuf::from(path), Some(name))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_nested_and_array() {
        let mut v = serde_json::json!({});
        set_in(
            &mut v,
            &parse_path("scripts.test"),
            Value::String("x".into()),
        );
        assert_eq!(v["scripts"]["test"], "x");
        set_in(
            &mut v,
            &parse_path("files[].name"),
            Value::String("a".into()),
        );
        assert_eq!(v["files"][0]["name"], "a");
        set_in(
            &mut v,
            &parse_path("keywords[0]"),
            Value::String("k".into()),
        );
        assert_eq!(v["keywords"][0], "k");
    }

    #[test]
    fn delete_nested() {
        let mut v = serde_json::json!({"scripts": {"test": "x", "build": "y"}});
        delete_in(&mut v, &parse_path("scripts.test"));
        assert!(v["scripts"].get("test").is_none());
        assert_eq!(v["scripts"]["build"], "y");
    }
}
