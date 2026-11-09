//! Port of `lib/commands/restart.js` (calls `run(['restart', ...args])`).

use crate::commands::command::{Command, CommandMeta};
use crate::commands::lifecycle_cmd::{LifecycleCommand, lifecycle_meta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = lifecycle_meta("restart", "Restart a package");

pub struct Restart;

impl Restart {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Restart {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Restart {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_lifecycle(npm, args)
    }
}

impl LifecycleCommand for Restart {}
