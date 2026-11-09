//! Replacement for `@npmcli/config`. Holds layered config values, the parsed
//! argv and the derived prefix/cache/dir paths that commands read.

use crate::utils::errors::{NpmError, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Registry used when no other is configured.
pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org/";

/// Where a config value was defined, from lowest to highest precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    Default = 0,
    Builtin = 1,
    Global = 2,
    User = 3,
    Project = 4,
    Cli = 5,
}

impl ConfigSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigSource::Default => "default",
            ConfigSource::Builtin => "builtin",
            ConfigSource::Global => "global",
            ConfigSource::User => "user",
            ConfigSource::Project => "project",
            ConfigSource::Cli => "cli",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub value: Value,
    pub source: ConfigSource,
    pub source_path: Option<PathBuf>,
}

/// A config key reported by `Config::get_unknown_configs`.
#[derive(Debug, Clone)]
pub struct UnknownConfig {
    pub key: String,
    pub base_key: String,
    pub where_: ConfigSource,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedArgv {
    /// Arguments after removing the command name.
    pub remain: Vec<String>,
    /// Full argv as nopt would return it.
    pub cooked: Vec<String>,
    pub unknown_flags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Store {
    entries: BTreeMap<String, ConfigEntry>,
    pub argv: Vec<String>,
    pub parsed_argv: ParsedArgv,
    pub loaded: bool,
    pub warn: bool,
    pub local_prefix: PathBuf,
    pub global_prefix: PathBuf,
    pub local_package: Option<PathBuf>,
    unknown_positionals: Vec<String>,
}

impl Store {
    pub fn new(local_prefix: PathBuf, argv: Vec<String>) -> Self {
        let mut store = Self {
            argv,
            local_prefix,
            global_prefix: global_prefix_dir(),
            ..Default::default()
        };
        store.apply_defaults();
        store
    }

    fn apply_defaults(&mut self) {
        self.define("registry", Value::String("https://registry.npmjs.org/".into()));
        self.define("cache", Value::String(default_cache_dir()));
        self.define("logspeed", Value::from(0));
        self.define("loglevel", Value::String("notice".into()));
        self.define("logs-max", Value::from(10));
        self.define("update-notifier", Value::Bool(true));
        self.define("progress", Value::Bool(true));
        self.define("color", Value::String("auto".into()));
        self.define("unicode", Value::Bool(true));
        self.define("json", Value::Bool(false));
        self.define("heading", Value::String("npm".into()));
        self.define("global", Value::Bool(false));
        self.define("force", Value::Bool(false));
        self.define("workspaces", Value::Bool(false));
        self.define("workspace", Value::Array(Vec::new()));
        self.define("usage", Value::Bool(false));
        self.define("timing", Value::Bool(false));
        self.define("yes", Value::Null);
        self.define("dry-run", Value::Bool(false));
        self.define("include-workspace-root", Value::Bool(false));
        self.define("install-links", Value::Bool(false));
        self.define("strict-npmrc", Value::Bool(false));
        self.define("extension-file", Value::Null);
        self.define("location", Value::String("user".into()));
        self.define("viewer", Value::String("man".into()));
        self.define("long", Value::Bool(false));
        self.define("audit", Value::Bool(true));
        self.define("all", Value::Bool(false));
        self.define("allow-scripts", Value::Null);
        self.define("allow-scripts-pin", Value::Bool(true));
        self.define("allow-scripts-pending", Value::Bool(false));
        self.define("dangerously-allow-all-scripts", Value::Bool(false));
        self.define("workspaces-update", Value::Bool(true));
        self.define("userconfig", Value::String(user_config_path()));
    }

    fn define(&mut self, key: &str, value: Value) {
        self.entries.insert(
            key.to_string(),
            ConfigEntry {
                value,
                source: ConfigSource::Default,
                source_path: None,
            },
        );
    }

    /// Loads config files and parses argv. In the Rust port this only records
    /// argv; the `.npmrc` parser is provided by [`crate::store::load_ini`].
    pub fn load(&mut self) -> Result<()> {
        self.load_cli()?;
        self.loaded = true;
        Ok(())
    }

    fn load_cli(&mut self) -> Result<()> {
        let mut remain = Vec::new();
        let mut cooked = Vec::new();
        let mut unknown_flags = Vec::new();
        let mut iter = self.argv.clone().into_iter().skip(1);
        while let Some(arg) = iter.next() {
            cooked.push(arg.clone());
            if let Some(flag) = arg.strip_prefix("--") {
                if flag.is_empty() || flag == "json" || flag == "usage" {
                    continue;
                }
                if let Some((key, value)) = flag.split_once('=') {
                    self.set(parse_value(value), key, ConfigSource::Cli);
                } else if matches!(flag, "versions" | "version" | "global" | "force" | "json") {
                    self.set(Value::Bool(true), flag, ConfigSource::Cli);
                } else if let Some(next) = iter.next() {
                    cooked.push(next.clone());
                    self.set(parse_value(&next), flag, ConfigSource::Cli);
                } else {
                    unknown_flags.push(flag.to_string());
                }
            } else {
                remain.push(arg);
            }
        }
        self.parsed_argv = ParsedArgv {
            remain,
            cooked,
            unknown_flags,
        };
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key).map(|e| &e.value)
    }

    /// Reads a value defined at or below `max` precedence, matching the
    /// `config.get(key, 'cli')` calls in `Npm::load`.
    pub fn get_at(&self, key: &str, max: ConfigSource) -> Option<&Value> {
        self.entries
            .get(key)
            .and_then(|e| (e.source <= max).then_some(&e.value))
    }

    pub fn is_default(&self, key: &str) -> bool {
        self.entries
            .get(key)
            .map(|e| e.source == ConfigSource::Default)
            .unwrap_or(true)
    }

    pub fn find(&self, key: &str) -> Option<ConfigSource> {
        self.entries.get(key).map(|e| e.source)
    }

    pub fn set(&mut self, value: Value, key: &str, source: ConfigSource) {
        let normalize = key.trim_start_matches("no-").replace('_', "-");
        let key = if key.starts_with("no-") && value.as_bool() == Some(true) {
            self.entries.insert(
                normalize.clone(),
                ConfigEntry {
                    value: Value::Bool(false),
                    source,
                    source_path: None,
                },
            );
            return;
        } else {
            normalize
        };
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.source > source {
                return;
            }
            entry.value = value;
            entry.source = source;
        } else {
            self.entries.insert(
                key,
                ConfigEntry {
                    value,
                    source,
                    source_path: None,
                },
            );
        }
    }

    pub fn get_str(&self, key: &str) -> String {
        match self.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        }
    }

    pub fn get_bool(&self, key: &str) -> bool {
        match self.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => matches!(s.as_str(), "true" | "1" | "yes"),
            Some(Value::Number(n)) => n.as_f64().map(|n| n != 0.0).unwrap_or(false),
            _ => false,
        }
    }

    pub fn get_list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                .collect(),
            Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        }
    }

    pub fn get_int(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
            Some(Value::String(s)) => s.parse().unwrap_or(0),
            Some(Value::Bool(true)) => 1,
            _ => 0,
        }
    }

    pub fn global(&self) -> bool {
        self.get_bool("global") || self.get_str("location") == "global"
    }

    pub fn cache(&self) -> PathBuf {
        let cache = self.get_str("cache");
        if cache.is_empty() {
            PathBuf::from(default_cache_dir())
        } else {
            PathBuf::from(cache)
        }
    }

    pub fn global_dir(&self) -> PathBuf {
        if cfg!(windows) {
            self.global_prefix.join("node_modules")
        } else {
            self.global_prefix.join("lib").join("node_modules")
        }
    }

    pub fn local_dir(&self) -> PathBuf {
        self.local_prefix.join("node_modules")
    }

    pub fn dir(&self) -> PathBuf {
        if self.global() {
            self.global_dir()
        } else {
            self.local_dir()
        }
    }

    pub fn global_bin(&self) -> PathBuf {
        if cfg!(windows) {
            self.global_prefix.clone()
        } else {
            self.global_prefix.join("bin")
        }
    }

    pub fn local_bin(&self) -> PathBuf {
        self.dir().join(".bin")
    }

    pub fn bin(&self) -> PathBuf {
        if self.global() {
            self.global_bin()
        } else {
            self.local_bin()
        }
    }

    pub fn prefix(&self) -> PathBuf {
        if self.global() {
            self.global_prefix.clone()
        } else {
            self.local_prefix.clone()
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.get_bool("workspaces") && !self.get_list("workspace").is_empty() {
            return Err(NpmError::new(
                "Cannot use --no-workspaces and --workspace at the same time",
            ));
        }
        Ok(())
    }

    pub fn get_unknown_configs(&self, where_: ConfigSource) -> Vec<UnknownConfig> {
        self.entries
            .iter()
            .filter(|(_, e)| e.source == where_ && e.source_path.is_none())
            .map(|(key, _)| UnknownConfig {
                key: key.clone(),
                base_key: key.clone(),
                where_,
                source: where_.as_str().to_string(),
            })
            .collect()
    }

    pub fn get_unknown_positionals(&self) -> &[String] {
        &self.unknown_positionals
    }

    pub fn remove_unknown_positional(&mut self, positional: &str) {
        self.unknown_positionals.retain(|p| p != positional);
    }

    pub fn log_warnings(&self) {
        for flag in &self.parsed_argv.unknown_flags {
            crate::utils::log::warn("config", &[&format!("Unknown cli config \"--{flag}\"")]);
        }
    }

    /// Entries whose value was defined exactly at `source`.
    pub fn entries_at(&self, source: ConfigSource) -> Vec<(&String, &ConfigEntry)> {
        self.entries
            .iter()
            .filter(|(_, e)| e.source == source)
            .collect()
    }

    /// Removes `key` when the entry was defined at `source`.
    pub fn remove(&mut self, key: &str, source: ConfigSource) {
        let key = key.trim_start_matches("no-").replace('_', "-");
        if self.entries.get(&key).is_some_and(|e| e.source == source) {
            self.entries.remove(&key);
        }
    }

    /// Configuration problems `npm config fix` can repair.
    pub fn repairable_problems(&self) -> Vec<RepairProblem> {
        self.entries
            .iter()
            .filter(|(key, _)| key.contains('_'))
            .map(|(key, entry)| RepairProblem {
                action: "rename",
                key: key.clone(),
                from: Some(key.clone()),
                to: Some(key.replace('_', "-")),
                where_: entry.source,
            })
            .collect()
    }

    /// Applies the renames/deletions reported by [`Self::repairable_problems`].
    pub fn repair(&mut self, problems: &[RepairProblem]) {
        for problem in problems {
            let Some(entry) = self.entries.remove(&problem.key) else {
                continue;
            };
            if problem.action == "rename" {
                if let Some(to) = &problem.to {
                    self.entries.insert(to.clone(), entry);
                }
            }
        }
    }

    pub fn flat(&self) -> BTreeMap<String, Value> {
        self.entries
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect()
    }
}

fn parse_value(raw: &str) -> Value {
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => match raw.parse::<i64>() {
            Ok(n) => Value::from(n),
            Err(_) => Value::String(raw.to_string()),
        },
    }
}

fn default_cache_dir() -> String {
    let base = crate::utils::paths::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("npm").to_string_lossy().into_owned()
}

fn user_config_path() -> String {
    crate::utils::paths::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".npmrc")
        .to_string_lossy()
        .into_owned()
}

fn global_prefix_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::utils::paths::home_dir().unwrap_or_default())
            .join("npm")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/usr/local")
    }
}

/// Parses an ini-style `.npmrc` file into the store. Keys keep their
/// source path so later diagnostics can point at the file.
pub fn load_ini(store: &mut Store, path: &Path, source: ConfigSource) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((key, value)) = line.split_once(['=', ':']) {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            store.set(Value::String(value.to_string()), key, source);
            if let Some(entry) = store.entries.get_mut(&key.replace('_', "-")) {
                entry.source_path = Some(path.to_path_buf());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_flag_wins() {
        let mut store = Store::new(PathBuf::from("."), vec!["npm".into(), "--loglevel=warn".into()]);
        store.load().unwrap();
        assert_eq!(store.get_str("loglevel"), "warn");
    }

    #[test]
    fn no_prefix_negates() {
        let mut store = Store::new(PathBuf::from("."), vec!["npm".into(), "--no-json".into()]);
        store.load().unwrap();
        assert!(!store.get_bool("json"));
    }
}
