//! Port of `lib/commands/uninstall.js`.

use std::path::Path;

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::reify_finish::ArboristRun;

pub static META: CommandMeta = CommandMeta::new("uninstall", "Remove a package")
    .with_params(&["save", "global"])
    .with_usage(&["[<@scope>/]<pkg>..."])
    .arborist();

pub struct Uninstall;

impl Uninstall {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Uninstall {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Uninstall {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let mut args = args.to_vec();
        if args.is_empty() {
            if !npm.global() {
                return Err(NpmError::new("Must provide a package name to remove"));
            }
            let prefix = npm.local_prefix();
            let manifest_path = prefix.join("package.json");
            if std::fs::metadata(&manifest_path).is_err() {
                return Err(META.usage_error(""));
            }
            let manifest: serde_json::Value = crate::cache::read_json(&manifest_path)?;
            let name = manifest
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            args.push(name);
        }

        let where_ = if npm.global() {
            npm.global_dir()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| npm.prefix())
        } else {
            npm.local_prefix().to_path_buf()
        };

        ArboristRun {
            prefix: where_,
            add: &[],
            remove: &args,
            update: &[],
            save: None,
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }
}

impl ArboristCommand for Uninstall {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
