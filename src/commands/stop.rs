//! Port of `lib/commands/stop.js` (calls `run(['stop', ...args])`).

use crate::commands::command::{Command, CommandMeta};
use crate::commands::lifecycle_cmd::{LifecycleCommand, lifecycle_meta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = lifecycle_meta("stop", "Stop a package");

pub struct Stop;

impl Stop {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Stop {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Stop {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_lifecycle(npm, args)
    }
}

impl LifecycleCommand for Stop {}
