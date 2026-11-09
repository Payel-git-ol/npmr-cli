//! Port of `lib/commands/patch.js`. `ls` and `rm` work against
//! `package.json#patchedDependencies`; `add`, `commit` and `update` need the
//! arborist tree and tarball handling and report `ENOTSUP`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new(
    "patch",
    "Apply local patches to installed dependencies",
)
.with_params(&[
    "patches-dir",
    "allow-unused-patches",
    "ignore-patch-failures",
    "edit-dir",
    "ignore-existing",
    "keep-edit-dir",
    "to",
    "registry",
])
.with_usage(&[
    "<pkg>[@<version>]",
    "add <pkg>[@<version>] [--edit-dir <path>] [--ignore-existing]",
    "commit <edit-dir> [--patches-dir <dir>] [--keep-edit-dir]",
    "update <pkg>[@<old-version>] [--to <new-version>] [--patches-dir <dir>]",
    "ls",
    "rm <pkg>[@<version>]",
]);

const SUBCOMMANDS: &[&str] = &["add", "commit", "update", "ls", "rm"];

/// Splits a `<name>@<spec>` selector into its parts.
fn parse_selector(key: &str) -> (String, String) {
    match key.rfind('@') {
        Some(0) | None => (key.to_string(), String::new()),
        Some(idx) => (key[..idx].to_string(), key[idx + 1..].to_string()),
    }
}

/// Splits a `<name>[@<version>]` argument into name and optional version.
fn parse_spec(spec: &str) -> (String, Option<String>) {
    let (name, raw) = parse_selector(spec);
    let version = if raw.is_empty() || raw == "*" {
        None
    } else {
        Some(raw)
    };
    (name, version)
}

fn read_root_manifest(root: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(root.join("package.json"))
        .map_err(|_| NpmError::new("no package.json found").with_code("ENOENT"))?;
    Ok(serde_json::from_str(&text)?)
}

fn patched_dependencies(manifest: &Value) -> serde_json::Map<String, Value> {
    manifest
        .get("patchedDependencies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub struct Patch;

impl Patch {
    pub fn new() -> Self {
        Self
    }

    fn add(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        let Some(spec) = args.first().filter(|a| !a.is_empty()) else {
            return Err(META.usage_error(""));
        };
        if spec.contains("://") || spec.starts_with("git") || spec.starts_with("file:") {
            return Err(NpmError::new(format!(
                "Only registry dependencies can be patched: {spec}"
            ))
            .with_code("EPATCHNONREGISTRY"));
        }
        let _ignore_existing = npm.store().get_bool("ignore-existing");
        Err(NpmError::new(format!(
            "Cannot add a patch for {spec}: extracting packages is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }

    fn commit(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        let Some(edit_dir) = args.first().filter(|a| !a.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let _patches_dir = npm.store().get_str("patches-dir");
        let _keep = npm.store().get_bool("keep-edit-dir");
        let edit_path = PathBuf::from(edit_dir);
        if !edit_path.is_dir() {
            return Err(NpmError::new(format!("Patch directory {} does not exist", edit_path.display()))
                .with_code("EPATCHEDITDIR"));
        }
        Err(NpmError::new(
            "Cannot commit a patch: extracting the original package is not implemented yet",
        )
        .with_code("ENOTSUP"))
    }

    fn update(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        let _to = npm.store().get_str("to");
        let _patches_dir = npm.store().get_str("patches-dir");
        let spec = args.first().cloned().unwrap_or_default();
        Err(NpmError::new(format!(
            "Cannot update the patch for {spec}: extracting packages is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }

    fn ls(&self, npm: &dyn Npm) -> Result<()> {
        let root = npm.local_prefix().to_path_buf();
        let manifest = read_root_manifest(&root)?;
        let patched = patched_dependencies(&manifest);
        for (key, path) in patched {
            let path = path.as_str().unwrap_or_default();
            // Node counts are only known once the install tree is loaded.
            crate::output::standard(format!("{path}\t{key}\t(0 nodes)"));
        }
        Ok(())
    }

    fn rm(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        if args.len() != 1 {
            return Err(META.usage_error(""));
        }
        let (target_name, target_version) = parse_spec(&args[0]);
        let root = npm.local_prefix().to_path_buf();
        let mut manifest = read_root_manifest(&root)?;
        let mut patched = patched_dependencies(&manifest);

        let removed: Vec<String> = patched
            .keys()
            .filter(|key| {
                let (name, spec) = parse_selector(key);
                name == target_name
                    && target_version
                        .as_ref()
                        .is_none_or(|version| &spec == version)
            })
            .cloned()
            .collect();
        if removed.is_empty() {
            return Err(
                NpmError::new(format!("No registered patch found for \"{}\".", args[0]))
                    .with_code("EPATCHNOTFOUND"),
            );
        }

        for key in &removed {
            let patch_path = patched
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            patched.remove(key);
            let still_used = patched.values().any(|v| v.as_str() == Some(patch_path.as_str()));
            if !still_used {
                let abs = root.join(&patch_path);
                let relative = abs
                    .strip_prefix(&root)
                    .map(|p| !p.as_os_str().is_empty() && !p.starts_with(".."))
                    .unwrap_or(false);
                if !relative {
                    return Err(NpmError::new(format!(
                        "Refusing to delete patch outside the project root: {patch_path}"
                    ))
                    .with_code("EPATCHUNSAFE"));
                }
                let _ = std::fs::remove_file(&abs);
            }
        }

        if let Value::Object(ref mut map) = manifest {
            if patched.is_empty() {
                map.remove("patchedDependencies");
            } else {
                map.insert("patchedDependencies".into(), Value::Object(patched));
            }
        }
        let text = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(root.join("package.json"), format!("{text}\n"))?;

        let plural = if removed.len() == 1 { "" } else { "es" };
        crate::output::standard(format!("Removed patch{plural}: {}", removed.join(", ")));
        Err(NpmError::new(
            "Patches were removed from package.json, but reinstalling requires the arborist tree, which is not implemented yet",
        )
        .with_code("ENOTSUP"))
    }
}

impl Default for Patch {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Patch {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(sub) = args.first().filter(|a| !a.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        if SUBCOMMANDS.contains(&sub.as_str()) {
            match sub.as_str() {
                "add" => self.add(&*npm, rest),
                "commit" => self.commit(&*npm, rest),
                "update" => self.update(&*npm, rest),
                "ls" => self.ls(&*npm),
                "rm" => self.rm(&*npm, rest),
                _ => Err(META.usage_error("")),
            }
        } else {
            // A bare argument is `patch add <pkg>`.
            self.add(&*npm, args)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_selector, parse_spec};

    #[test]
    fn parses_selectors_and_specs() {
        assert_eq!(parse_selector("lodash@4.17.21"), ("lodash".into(), "4.17.21".into()));
        assert_eq!(parse_selector("@a/b@^1"), ("@a/b".into(), "^1".into()));
        assert_eq!(parse_spec("lodash"), ("lodash".into(), None));
        assert_eq!(parse_spec("lodash@*"), ("lodash".into(), None));
        assert_eq!(
            parse_spec("lodash@4"),
            ("lodash".into(), Some("4".into()))
        );
    }
}
