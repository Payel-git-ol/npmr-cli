//! Port of `lib/commands/unpublish.js`. Argument and safety validation
//! (`--force` for whole-project removal) is ported; the registry calls report
//! `ENOTSUP`.

use std::path::Path;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::silly;

pub static META: CommandMeta = CommandMeta::new("unpublish", "Remove a package from the registry")
    .with_params(&["dry-run", "force", "workspace", "workspaces"])
    .with_usage(&["[<package-spec>]"])
    .workspaces();


/// Splits `name@spec`; `None` means the whole project.
fn parse_spec(arg: &str) -> (String, Option<String>) {
    match arg.rfind('@') {
        Some(0) | None => (arg.to_string(), None),
        Some(idx) => (arg[..idx].to_string(), Some(arg[idx + 1..].to_string())),
    }
}

pub struct Unpublish;

impl Unpublish {
    pub fn new() -> Self {
        Self
    }

    fn read_manifest(local_prefix: &Path) -> Option<serde_json::Value> {
        std::fs::read_to_string(local_prefix.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    }

    fn unpublish(&self, npm: &dyn Npm, args: &[String], local_prefix: &Path) -> Result<()> {
        if args.len() > 1 {
            return Err(META.usage_error(""));
        }
        let force = npm.store().get_bool("force");
        let _dry_run = npm.store().get_bool("dry-run");

        let spec = match args.first() {
            Some(arg) => {
                let (name, version) = parse_spec(arg);
                match version {
                    Some(version) if version != "*" && semver::Version::parse(&version).is_err() => {
                        return Err(META.usage_error(
                            "Can only unpublish a single version, or the entire project.\nTags and ranges are not supported.",
                        ));
                    }
                    version => Some((name, version)),
                }
            }
            None => None,
        };
        silly("unpublish", &[&"args[0]", &args.first().cloned().unwrap_or_default()]);

        if spec.as_ref().and_then(|(_, v)| v.as_deref()) == Some("*") && !force {
            return Err(META.usage_error(
                "Refusing to delete entire project.\nRun with --force to do this.",
            ));
        }

        let manifest = Self::read_manifest(local_prefix);
        let target = match spec {
            Some((name, version)) => (name, version.map(|v| format!("@{v}")).unwrap_or_default()),
            None => {
                let Some(manifest) = manifest else {
                    return Err(META.usage_error(""));
                };
                let name = manifest
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let version = manifest
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if version.is_empty() && !force {
                    return Err(META.usage_error(
                        "Refusing to delete entire project.\nRun with --force to do this.",
                    ));
                }
                (name, format!("@{version}"))
            }
        };
        let (name, version) = target;
        Err(NpmError::new(format!(
            "Cannot unpublish {name}{version}: registry access is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}

impl Default for Unpublish {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Unpublish {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let prefix = npm.local_prefix().to_path_buf();
        self.unpublish(&*npm, args, &prefix)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        for path in workspaces.values() {
            self.unpublish(&*npm, args, Path::new(path))?;
        }
        Ok(())
    }
}
