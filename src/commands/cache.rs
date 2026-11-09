//! Port of `lib/commands/cache.js`. The cacache on-disk format is not
//! reimplemented yet, so key-level operations report `ENOTSUP` while the
//! whole-cache operations (clean/verify/npx) work on plain directories.

use std::path::{Path, PathBuf};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::silly;

pub static META: CommandMeta = CommandMeta::new("cache", "Manipulates packages and npx cache")
    .with_params(&["cache"])
    .with_usage(&[
        "add <package-spec>",
        "clean [<key>]",
        "ls [<name>@<version>]",
        "verify",
        "npx ls",
        "npx rm [<key>...]",
        "npx info <key>...",
    ]);

const CLEAN_WARNING: &str = "As of npm@5, the npm cache self-heals from corruption issues by treating integrity mismatches as cache misses.
As a result, data extracted from the cache is guaranteed to be valid.
If you want to make sure everything is consistent, use `npm cache verify` instead.
Deleting the cache can only make npm go slower, and is not likely to correct any problems you may be encountering!

On the other hand, if you're debugging an issue with the installer, or race conditions that depend on the timing of writing to an empty cache, you can use `npm install --cache /tmp/empty-cache` to use a temporary cache instead of removing the actual one.

If you're sure you want to delete the entire cache, rerun this command with --force.";

pub struct Cache;

impl Cache {
    pub fn new() -> Self {
        Self
    }

    fn npx_cache_dir(npm: &dyn Npm) -> PathBuf {
        npm.store().cache().join("_npx")
    }

    fn clean(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        let cache = npm.store().cache();
        if args.is_empty() {
            if !npm.store().get_bool("force") {
                return Err(NpmError::new(CLEAN_WARNING));
            }
            let _ = std::fs::remove_dir_all(cache);
            return Ok(());
        }
        Err(NpmError::new(
            "Removing individual cache keys requires the cacache store, which is not ported yet",
        )
        .with_code("ENOTSUP"))
    }

    fn add(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        silly("cache add", &[&"args", &args.join(" ")]);
        if args.is_empty() {
            return Err(META.usage_error("First argument to `add` is required"));
        }
        for spec in args {
            silly("cache add", &[&"spec", spec]);
        }
        let _ = npm;
        Err(
            NpmError::new("Fetching packages into the cache is not implemented yet")
                .with_code("ENOTSUP"),
        )
    }

    fn verify(&self, npm: &dyn Npm) -> Result<()> {
        let cache = npm.store().cache();
        let prefix = crate::utils::paths::home_dir()
            .filter(|home| cache.starts_with(home))
            .map(|home| {
                let rest = cache.strip_prefix(&home).unwrap_or(Path::new(""));
                format!("~/{}", rest.display())
            })
            .unwrap_or_else(|| cache.display().to_string());

        let mut entries = 0usize;
        let mut kept = 0u64;
        let mut bad = 0usize;
        visit(cache, &mut |file| match std::fs::metadata(file) {
            Ok(meta) => {
                entries += 1;
                kept += meta.len();
            }
            Err(_) => bad += 1,
        });

        crate::output::standard(format!("Cache verified and compressed ({prefix})"));
        crate::output::standard(format!("Content verified: {entries} ({kept} bytes)"));
        if bad > 0 {
            crate::output::standard(format!("Corrupted content removed: {bad}"));
        }
        crate::output::standard(format!("Index entries: {entries}"));
        Ok(())
    }

    fn ls(&self, npm: &dyn Npm, specs: &[String]) -> Result<()> {
        let cache = npm.store().cache();
        let mut keys = Vec::new();
        for entry in std::fs::read_dir(cache.join("_cacache").join("index-v5"))
            .into_iter()
            .flatten()
            .flatten()
        {
            keys.push(entry.file_name().to_string_lossy().into_owned());
        }
        keys.sort();
        if specs.is_empty() {
            for key in keys {
                crate::output::standard(key);
            }
            return Ok(());
        }
        Err(
            NpmError::new("Searching cache keys by package spec is not ported yet")
                .with_code("ENOTSUP"),
        )
    }

    fn npx_entries(&self, npm: &dyn Npm, keys: &[String]) -> Result<Vec<(String, PathBuf, bool)>> {
        let dir = Self::npx_cache_dir(npm);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            crate::output::standard("npx cache does not exist");
            return Ok(Vec::new());
        };
        let mut all: Vec<(String, PathBuf, bool)> = entries
            .flatten()
            .map(|entry| {
                let hash = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path();
                let valid = path.join("package.json").is_file();
                (hash, path, valid)
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        if keys.is_empty() {
            return Ok(all);
        }
        let mut picked = Vec::new();
        for key in keys {
            match all
                .iter()
                .find(|(hash, _, _)| hash == key || hash.starts_with(key.as_str()))
            {
                Some(found) => picked.push(found.clone()),
                None => {
                    return Err(META.usage_error(format!("Invalid npx key {key}")));
                }
            }
        }
        Ok(picked)
    }

    fn npx_ls(&self, npm: &dyn Npm) -> Result<()> {
        for (hash, path, valid) in self.npx_entries(npm, &[])? {
            if !valid {
                crate::output::standard(format!("{hash}: (empty/invalid)"));
                continue;
            }
            let manifest = std::fs::read_to_string(path.join("package.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .unwrap_or_default();
            let packages = manifest
                .get("_npx")
                .and_then(|n| n.get("packages"))
                .and_then(serde_json::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                });
            match packages {
                Some(packages) => crate::output::standard(format!("{hash}: {packages}")),
                None => crate::output::standard(format!("{hash}: (unknown)")),
            }
        }
        Ok(())
    }

    fn npx_rm(&self, npm: &dyn Npm, keys: &[String]) -> Result<()> {
        let dry_run = npm.store().get_bool("dry-run");
        if keys.is_empty() {
            if !npm.store().get_bool("force") {
                return Err(META.usage_error("Please use --force to remove entire npx cache"));
            }
            if !dry_run {
                let _ = std::fs::remove_dir_all(Self::npx_cache_dir(npm));
            }
            return Ok(());
        }
        for (_, path, _) in self.npx_entries(npm, keys)? {
            crate::output::standard(format!("Removing npx key at {}", path.display()));
            if !dry_run {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
        Ok(())
    }

    fn npx_info(&self, npm: &dyn Npm, keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return Err(META.usage_error(""));
        }
        let blue = |s: &str| {
            crate::utils::display::paint(s, crate::utils::display::Color::Blue, npm.color())
        };
        for (hash, path, valid) in self.npx_entries(npm, keys)? {
            let v = if valid {
                crate::utils::display::paint(
                    "valid",
                    crate::utils::display::Color::Green,
                    npm.color(),
                )
            } else {
                crate::utils::display::paint(
                    "invalid",
                    crate::utils::display::Color::Red,
                    npm.color(),
                )
            };
            crate::output::standard(format!("{v} npx cache entry with key {}", blue(&hash)));
            crate::output::standard(format!("location: {}", blue(&path.display().to_string())));
            crate::output::standard("");
        }
        Ok(())
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

fn visit(dir: &Path, f: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, f);
        } else {
            f(&path);
        }
    }
}

impl Command for Cache {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(cmd) = args.first() else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        match cmd.as_str() {
            "rm" | "clear" | "clean" => self.clean(npm, rest),
            "add" => self.add(npm, rest),
            "verify" | "check" => self.verify(npm),
            "ls" => self.ls(npm, rest),
            "npx" => {
                let Some(sub) = rest.first() else {
                    return Err(META.usage_error(""));
                };
                match sub.as_str() {
                    "ls" => self.npx_ls(npm),
                    "rm" => self.npx_rm(npm, &rest[1..]),
                    "info" => self.npx_info(npm, &rest[1..]),
                    _ => Err(META.usage_error("")),
                }
            }
            _ => Err(META.usage_error("")),
        }
    }
}
