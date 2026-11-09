//! Port of `lib/commands/ci.js`: clean install driven purely by the lockfile.

use std::path::Path;

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::verbose;
use crate::utils::reify_finish::ArboristRun;

pub static CI_PARAMS: &[&str] = &[
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "omit",
    "include",
    "strict-peer-deps",
    "foreground-scripts",
    "ignore-scripts",
    "allow-directory",
    "allow-file",
    "allow-git",
    "allow-remote",
    "allow-scripts",
    "strict-allow-scripts",
    "dangerously-allow-all-scripts",
    "audit",
    "bin-links",
    "fund",
    "dry-run",
];

pub static META: CommandMeta = CommandMeta::new("ci", "Clean install a project")
    .with_params(CI_PARAMS)
    .arborist();

const NO_LOCKFILE: &str = "The `npm ci` command can only install with an existing\n\
package-lock.json with lockfileVersion >= 1. Run an install with npm@5\n\
or later to generate a package-lock.json file, then try again.";

pub struct Ci;

impl Ci {
    pub fn new() -> Self {
        Self
    }

    /// Loads and validates the lockfile, returning the locked package map.
    pub fn validate(npm: &dyn Npm) -> Result<serde_json::Value> {
        let prefix = npm.prefix();
        let text = std::fs::read_to_string(prefix.join("package-lock.json")).map_err(|err| {
            verbose("loadVirtual", &[&err.to_string()]);
            META.usage_error(NO_LOCKFILE)
        })?;
        let lock: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
            verbose("loadVirtual", &[&err.to_string()]);
            META.usage_error(NO_LOCKFILE)
        })?;
        let version = lock.get("lockfileVersion").and_then(serde_json::Value::as_u64).unwrap_or(0);
        if version < 1 {
            verbose("loadVirtual", &[&"lockfileVersion < 1"]);
            return Err(META.usage_error(NO_LOCKFILE));
        }

        let inventory = crate::utils::validate_lockfile::lockfile_inventory(&lock);
        let ideal = read_ideal_inventory(&prefix);
        let mut errors = crate::utils::validate_lockfile::validate_lockfile(&inventory, &ideal);
        if let Ok(manifest_text) = std::fs::read_to_string(prefix.join("package.json")) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&manifest_text) {
                errors.extend(
                    crate::utils::validate_lockfile::validate_package_extensions(&lock, &manifest),
                );
            }
        }
        errors.extend(crate::utils::validate_lockfile::validate_npm_extension(&lock, None));
        if !errors.is_empty() {
            return Err(META.usage_error(&format!(
                "`npm ci` can only install packages when your package.json and package-lock.json are in sync. \
                 Please update your lock file with `npm install` before continuing.\n\n{}",
                errors.join("\n")
            )));
        }
        Ok(lock)
    }
}

/// The ideal inventory is just the currently installed tree until resolution
/// is ported; existing installs therefore validate byte-for-byte.
fn read_ideal_inventory(prefix: &Path) -> std::collections::BTreeMap<String, crate::utils::validate_lockfile::LockedNode> {
    let mut inventory = std::collections::BTreeMap::new();
    for node in crate::utils::allow_scripts_writer::scan_nodes(prefix) {
        let location = node
            .path
            .rsplit_once("node_modules")
            .map(|(_, rest)| format!("node_modules{rest}").replace('\\', "/"))
            .unwrap_or_else(|| node.path.clone());
        inventory.insert(
            location.clone(),
            crate::utils::validate_lockfile::LockedNode {
                location,
                name: node.name,
                version: node.version,
                resolved: node.resolved,
                integrity: None,
                dev: false,
                optional: false,
                link: false,
            },
        );
    }
    inventory
}

fn empty_node_modules(path: &Path) -> std::io::Result<()> {
    let modules = path.join("node_modules");
    let Ok(entries) = std::fs::read_dir(&modules) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let full = entry.path();
        if full.is_dir() && !full.is_symlink() {
            std::fs::remove_dir_all(&full)?;
        } else {
            std::fs::remove_file(&full)?;
        }
    }
    Ok(())
}

impl Command for Ci {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        if npm.global() {
            return Err(crate::commands::install::global_error("ci"));
        }

        for flag in crate::utils::cli_flags::PATCH_RELAX_FLAGS {
            if npm.store().find(flag) == Some(ConfigSource::Cli) {
                return Err(NpmError::new(format!(
                    "The --{flag} flag is not allowed with `npm ci`."
                ))
                .with_code("ECIPATCHFLAG"));
            }
        }

        let dry_run = npm.store().get_bool("dry-run");
        let ignore_scripts = npm.store().get_bool("ignore-scripts");
        let where_ = npm.prefix();

        Self::validate(npm)?;

        if !dry_run {
            empty_node_modules(&where_)?;
        }

        if !ignore_scripts {
            let _ = npm.exec("run", &["preinstall".to_string()]);
        }

        ArboristRun {
            prefix: where_.clone(),
            add: &[],
            remove: &[],
            update: &[],
            save: Some(false),
            dry_run,
            workspaces: &[],
            package_lock: Some(true),
        }
        .execute(npm)?;

        if !ignore_scripts {
            for event in [
                "install",
                "postinstall",
                "prepublish",
                "preprepare",
                "prepare",
                "postprepare",
            ] {
                let _ = npm.exec("run", &[event.to_string()]);
            }
        }
        Ok(())
    }
}

impl ArboristCommand for Ci {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
