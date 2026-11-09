//! Port of `lib/commands/prune.js`: removes extraneous packages.

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::reify_finish::ArboristRun;

pub static PRUNE_PARAMS: &[&str] = &[
    "omit",
    "include",
    "dry-run",
    "json",
    "foreground-scripts",
    "ignore-scripts",
];

pub static META: CommandMeta = CommandMeta::new("prune", "Remove extraneous packages")
    .with_params(PRUNE_PARAMS)
    .with_usage(&["[[<@scope>/]<pkg>...]"])
    .arborist();

pub struct Prune;

impl Prune {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Prune {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Prune {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    /// Arborist prunes by reifying the ideal tree with nothing to add.
    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        ArboristRun {
            prefix: npm.prefix(),
            add: &[],
            remove: &[],
            update: &[],
            save: Some(false),
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }
}

impl ArboristCommand for Prune {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
