//! Port of `lib/commands/install-ci-test.js`.

use crate::commands::ci::CI_PARAMS;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new(
    "install-ci-test",
    "Install a project with a clean slate and run tests",
)
.with_params(CI_PARAMS)
.arborist();

pub struct InstallCiTest;

impl InstallCiTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InstallCiTest {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for InstallCiTest {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        npm.exec("ci", args)?;
        npm.exec("test", &[])
    }
}
