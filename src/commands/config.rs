//! Port of `lib/commands/config.js`. Edits the layered `.npmrc` files through
//! `get`, `set`, `delete`, `list`, `edit` and `fix`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::{ConfigEntry, ConfigSource, Store};
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{info, warn};
use crate::utils::redact::redact;

pub static META: CommandMeta = CommandMeta::new("config", "Manage the npm configuration files")
    .with_params(&["json", "global", "editor", "location", "long"])
    .with_usage(&[
        "set <key>=<value> [<key>=<value> ...]",
        "get [<key> [<key> ...]]",
        "delete <key> [<key> ...]",
        "list [--json]",
        "edit",
        "fix",
    ])
    .skip_validation();

/// Values that always render as `(protected)`.
const PROTECTED: &[&str] = &[
    "auth",
    "authToken",
    "certfile",
    "email",
    "keyfile",
    "password",
    "username",
];

const VALID_OPTIONS: &[&str] = &[
    "registry",
    "cache",
    "loglevel",
    "logs-max",
    "update-notifier",
    "progress",
    "color",
    "unicode",
    "json",
    "heading",
    "global",
    "force",
    "workspaces",
    "workspace",
    "usage",
    "timing",
    "yes",
    "dry-run",
    "include-workspace-root",
    "install-links",
    "strict-npmrc",
    "extension-file",
    "location",
    "viewer",
    "long",
    "audit",
    "userconfig",
    "all",
    "allow-scripts",
    "allow-scripts-pin",
    "allow-scripts-pending",
    "dangerously-allow-all-scripts",
    "workspaces-update",
    "editor",
    "shell",
    "script-shell",
    "save",
    "save-dev",
    "save-peer",
    "save-optional",
    "save-prod",
    "save-exact",
    "ignore-scripts",
    "foreground-scripts",
    "depth",
    "parseable",
    "if-present",
    "package",
    "call",
    "browser",
    "sbom-format",
    "sbom-type",
    "tag",
    "access",
    "otp",
];

fn is_protected(key: &str) -> bool {
    if key.starts_with('_') {
        return true;
    }
    if PROTECTED.contains(&key) {
        return true;
    }
    if key.starts_with("//") {
        if key.contains(":_") {
            return true;
        }
        for p in PROTECTED {
            if key.ends_with(&format!(":{p}")) || key.ends_with(&format!(":_ {p}")) {
                return true;
            }
        }
    }
    false
}

fn is_private(key: &str, value: &str) -> bool {
    is_protected(key) || redact(value) != value
}

fn display_var(key: &str, value: &str) -> String {
    if is_protected(key) {
        format!("{key} = (protected)")
    } else {
        format!("{key} = {}", Value::String(redact(value)))
    }
}

/// Parses `[k, v, k2=v2, ...]` into key/value pairs.
pub fn key_values(args: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let (key, value) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => {
                let value = if i + 1 < args.len() {
                    i += 1;
                    args[i].clone()
                } else {
                    String::new()
                };
                (arg.clone(), value)
            }
        };
        out.push((key.trim().to_string(), value.trim().to_string()));
        i += 1;
    }
    out
}

fn config_path(store: &Store, where_: ConfigSource) -> Result<PathBuf> {
    match where_ {
        ConfigSource::User => Ok(PathBuf::from(store.get_str("userconfig"))),
        ConfigSource::Global | ConfigSource::Builtin => {
            Ok(store.global_prefix.join("etc").join("npmrc"))
        }
        ConfigSource::Project => Ok(Path::new(&store.local_prefix).join(".npmrc")),
        ConfigSource::Cli => Err(NpmError::new("Cannot edit the command line configuration")
            .with_code("ESAVECLI")),
        ConfigSource::Default => Ok(Path::new(&store.local_prefix).join(".npmrc")),
    }
}

fn source_for_location(store: &Store) -> ConfigSource {
    match store.get_str("location").as_str() {
        "global" => ConfigSource::Global,
        "project" => ConfigSource::Project,
        "user" => ConfigSource::User,
        _ if store.global() => ConfigSource::Global,
        _ => ConfigSource::User,
    }
}

/// Rewrites the ini file for `where` with every entry defined at that level.
pub fn save(store: &Store, where_: ConfigSource) -> Result<()> {
    let path = config_path(store, where_)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut body = String::new();
    for (key, entry) in store.entries_at(where_) {
        let value = match &entry.value {
            Value::String(s) => s.clone(),
            Value::Null => continue,
            other => other.to_string(),
        };
        body.push_str(&format!("{key}={value}\n"));
    }
    std::fs::write(path, body)?;
    Ok(())
}

pub struct Config;

impl Config {
    pub fn new() -> Self {
        Self
    }

    fn set(&self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(META.usage_error(""));
        }
        let where_ = source_for_location(npm.store());
        let pairs = key_values(args);
        for (key, value) in pairs {
            info("config", &[&format!("set {key} {value}")]);
            let base = key.rsplit(':').next().unwrap_or(&key).to_string();
            if !VALID_OPTIONS.contains(&base.as_str()) && !base.starts_with('_') {
                return Err(NpmError::new(format!("`{base}` is not a valid npm option")));
            }
            if value.is_empty() {
                npm.store_mut().remove(&key, where_);
            } else {
                npm.store_mut().set(Value::String(value), &key, where_);
            }
            if npm.store().validate().is_err() {
                warn("config", &[&"omitting invalid config values"]);
            }
        }
        save(npm.store(), where_)
    }

    fn get(&self, npm: &dyn Npm, keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return self.list(npm);
        }
        let store = npm.store();
        let mut out = Vec::new();
        for key in keys {
            let value = store.get_str(key);
            if is_private(key, &value) {
                return Err(NpmError::new(format!(
                    "The {key} option is protected, and cannot be retrieved in this way"
                )));
            }
            if keys.len() > 1 {
                out.push(format!("{key}={value}"));
            } else {
                out.push(value);
            }
        }
        crate::output::standard(out.join("\n"));
        Ok(())
    }

    fn delete(&self, npm: &mut (dyn Npm + '_), keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return Err(META.usage_error(""));
        }
        let where_ = source_for_location(npm.store());
        for key in keys {
            npm.store_mut().remove(key, where_);
        }
        save(npm.store(), where_)
    }

    fn edit(&self, npm: &dyn Npm) -> Result<()> {
        let where_ = source_for_location(npm.store());
        save(npm.store(), where_)?;
        let file = config_path(npm.store(), where_)?;
        let data = std::fs::read_to_string(&file)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let mut lines: Vec<String> = data.split('\n').map(str::to_string).collect();
        lines.sort();
        let header = format!(
            ";;;;
; npm {where_}config file: {}
; this is a simple ini-formatted file
; lines that start with semi-colons are comments
; run `npm help 7 config` for documentation of the various options
;
; Configs like `@scope:registry` map a scope to a given registry url.
;
; Configs like `//<hostname>/:_authToken` are auth that is restricted
; to the registry host specified.

{}

;;;;
; all available options shown below with default values
;;;;
",
            file.display(),
            lines.join("\n").trim(),
            where_ = where_.as_str(),
        );
        std::fs::write(&file, header)?;

        let editor = npm.store().get_str("editor");
        let mut words = editor.split_whitespace();
        let Some(bin) = words.next() else {
            return Err(NpmError::new("No editor configured").with_code("ENOEDITOR"));
        };
        let status = std::process::Command::new(bin)
            .args(words)
            .arg(&file)
            .status()
            .map_err(NpmError::from)?;
        if !status.success() {
            return Err(NpmError::new(format!(
                "editor process exited with code: {}",
                status.code().unwrap_or(1)
            )));
        }
        Ok(())
    }

    fn list(&self, npm: &dyn Npm) -> Result<()> {
        let store = npm.store();
        let long = store.get_bool("long");
        let mut msg: Vec<String> = Vec::new();
        for source in [
            ConfigSource::Cli,
            ConfigSource::Project,
            ConfigSource::User,
            ConfigSource::Global,
            ConfigSource::Builtin,
            ConfigSource::Default,
        ] {
            if source == ConfigSource::Default && !long {
                continue;
            }
            let mut entries: Vec<(&String, &ConfigEntry)> = store.entries_at(source);
            if entries.is_empty() {
                continue;
            }
            entries.sort_by(|a, b| a.0.cmp(b.0));
            msg.push(format!(
                "; \"{}\" config from {}",
                source.as_str(),
                config_path(store, source)
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ));
            msg.push(String::new());
            for (key, entry) in entries {
                let value = match &entry.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                let display = display_var(key, &value);
                match store.find(key) {
                    Some(found) if found == source => msg.push(display),
                    Some(found) => msg.push(format!(
                        "; {display} ; overridden by {}",
                        found.as_str()
                    )),
                    None => msg.push(display),
                }
            }
            msg.push(String::new());
        }

        if !long {
            msg.push(format!(
                "; npm local prefix = {}",
                store.local_prefix.display()
            ));
            msg.push(format!("; npm version = {}", npm.version()));
            if let Ok(cwd) = std::env::current_dir() {
                msg.push(format!("; cwd = {}", cwd.display()));
            }
            msg.push("; Run `npm config ls -l` to show all defaults.".into());
            msg.push(String::new());
        }

        if !npm.global() {
            let prefix = npm.prefix();
            if let Ok(text) = std::fs::read_to_string(prefix.join("package.json")) {
                if let Ok(manifest) = serde_json::from_str::<Value>(&text) {
                    if let Some(publish) = manifest.get("publishConfig").and_then(Value::as_object)
                    {
                        msg.push(format!(
                            "; \"publishConfig\" from {}",
                            prefix.join("package.json").display()
                        ));
                        msg.push(
                            "; This set of config values will be used at publish-time.".into(),
                        );
                        msg.push(String::new());
                        let mut entries: Vec<(&String, &Value)> = publish.iter().collect();
                        entries.sort_by(|a, b| a.0.cmp(b.0));
                        for (key, value) in entries {
                            let text = match value {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            msg.push(display_var(key, &text));
                        }
                        msg.push(String::new());
                    }
                }
            }
        }

        crate::output::standard(msg.join("\n").trim());
        Ok(())
    }

    fn list_json(&self, npm: &dyn Npm) {
        let store = npm.store();
        let mut public = serde_json::Map::new();
        for (key, value) in store.flat() {
            let text = match &value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if is_private(&key, &text) {
                continue;
            }
            public.insert(key, value);
        }
        crate::output::set_meta(true);
        crate::output::standard(Value::Object(public).to_string());
    }

    fn fix(&self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let problems = npm.store().repairable_problems();
        if problems.is_empty() {
            return Ok(());
        }
        let mut locations = Vec::new();
        let mut lines = Vec::new();
        for problem in &problems {
            match problem.action {
                "rename" => {
                    locations.push(problem.where_);
                    lines.push(format!(
                        "~ `{}` renamed to `{}` in {} config",
                        problem.key,
                        problem.to.as_deref().unwrap_or_default(),
                        problem.where_.as_str()
                    ));
                }
                _ => {
                    locations.push(problem.where_);
                    lines.push(format!(
                        "- `{}` deleted from {} config",
                        problem.key,
                        problem.where_.as_str()
                    ));
                }
            }
        }
        npm.store_mut().repair(&problems);
        crate::output::standard("The following configuration problems have been repaired:\n");
        crate::output::standard(lines.join("\n"));
        for location in locations {
            save(npm.store(), location)?;
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Config {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(action) = args.first() else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        match action.as_str() {
            "set" => self.set(npm, rest),
            "get" => self.get(npm, rest),
            "delete" | "rm" | "del" => self.delete(npm, rest),
            "list" | "ls" => {
                if npm.json() {
                    self.list_json(npm);
                    Ok(())
                } else {
                    self.list(npm)
                }
            }
            "edit" => self.edit(npm),
            "fix" => self.fix(npm),
            _ => Err(META.usage_error("")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_values_pairs_bare_and_equals() {
        let args = vec!["a=1".to_string(), "b".to_string(), "2".to_string(), "c".to_string()];
        assert_eq!(
            key_values(&args),
            vec![
                ("a".into(), "1".into()),
                ("b".into(), "2".into()),
                ("c".into(), String::new())
            ]
        );
    }

    #[test]
    fn auth_tokens_are_protected() {
        assert!(is_protected("//registry.npmjs.org/:_authToken"));
        assert!(is_protected("_password"));
        assert!(!is_protected("registry"));
    }
}
