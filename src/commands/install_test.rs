//! Port of `lib/commands/install-test.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::commands::install::INSTALL_PARAMS;
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("install-test", "Install package(s) and run tests")
    .with_params(INSTALL_PARAMS)
    .arborist();

pub struct InstallTest;

impl InstallTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InstallTest {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for InstallTest {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        npm.exec("install", args)?;
        npm.exec("test", &[])
    }
}
