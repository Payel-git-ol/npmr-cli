//! Port of `lib/commands/dedupe.js`.

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::reify_finish::ArboristRun;

pub static DEDUPE_PARAMS: &[&str] = &[
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "strict-peer-deps",
    "package-lock",
    "omit",
    "include",
    "ignore-scripts",
    "allow-directory",
    "allow-file",
    "allow-git",
    "allow-remote",
    "audit",
    "bin-links",
    "fund",
    "dry-run",
];

pub static META: CommandMeta = CommandMeta::new("dedupe", "Reduce duplication in the package tree")
    .with_params(DEDUPE_PARAMS)
    .arborist();

pub struct Dedupe;

impl Dedupe {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Dedupe {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Dedupe {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        if npm.global() {
            return Err(NpmError::new("`npm dedupe` does not work in global mode.")
                .with_code("EDEDUPEGLOBAL"));
        }
        ArboristRun {
            prefix: npm.prefix(),
            add: &[],
            remove: &[],
            update: &[],
            // Saving during dedupe would be confusing, see the JS command.
            save: Some(false),
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }
}

impl ArboristCommand for Dedupe {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
