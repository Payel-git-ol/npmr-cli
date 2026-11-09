//! Port of `lib/commands/update.js`.

use std::path::Path;

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::log::warn;
use crate::utils::reify_finish::ArboristRun;

pub static UPDATE_PARAMS: &[&str] = &[
    "save",
    "global",
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "omit",
    "include",
    "strict-peer-deps",
    "package-lock",
    "foreground-scripts",
    "ignore-scripts",
    "allow-scripts",
    "strict-allow-scripts",
    "dangerously-allow-all-scripts",
    "audit",
    "before",
    "min-release-age",
    "min-release-age-exclude",
    "bin-links",
    "fund",
    "dry-run",
];

pub static META: CommandMeta = CommandMeta::new("update", "Update packages")
    .with_params(UPDATE_PARAMS)
    .with_usage(&["[<pkg>...]"])
    .arborist();

pub struct Update;

impl Update {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Update {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Update {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let where_ = if npm.global() {
            npm.global_dir()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| npm.prefix())
        } else {
            npm.prefix()
        };
        let save = if npm.store().is_default("save") {
            Some(false)
        } else {
            Some(npm.store().get_bool("save"))
        };
        if npm.store().get_int("depth") != 0 {
            warn(
                "update",
                &[
                    &"The --depth option no longer has any effect. See RFC0019.\nhttps://github.com/npm/rfcs/blob/latest/implemented/0019-remove-update-depth-option.md",
                ],
            );
        }
        ArboristRun {
            prefix: where_,
            add: &[],
            remove: &[],
            update: args,
            save,
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }
}

impl ArboristCommand for Update {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
