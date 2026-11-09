//! Port of `lib/commands/find-dupes.js`: `dedupe` in dry-run mode.

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::Result;

pub static FIND_DUPES_PARAMS: &[&str] = &[
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "strict-peer-deps",
    "package-lock",
    "omit",
    "include",
    "ignore-scripts",
    "audit",
    "bin-links",
    "fund",
];

pub static META: CommandMeta =
    CommandMeta::new("find-dupes", "Find duplication in the package tree")
        .with_params(FIND_DUPES_PARAMS)
        .arborist();

pub struct FindDupes;

impl FindDupes {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FindDupes {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for FindDupes {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        npm.store_mut()
            .set(serde_json::Value::Bool(true), "dry-run", ConfigSource::Cli);
        npm.exec("dedupe", &[])
    }
}

impl ArboristCommand for FindDupes {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
