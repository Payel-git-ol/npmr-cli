//! Port of `lib/commands/test.js` (calls `run(['test', ...args])`).

use crate::commands::command::{Command, CommandMeta};
use crate::commands::lifecycle_cmd::{LifecycleCommand, lifecycle_meta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = lifecycle_meta("test", "Test a package");

pub struct Test;

impl Test {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Test {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Test {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_lifecycle(npm, args)
    }
}

impl LifecycleCommand for Test {}
