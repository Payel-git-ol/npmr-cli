//! Port of `lib/commands/start.js` (calls `run(['start', ...args])`).

use crate::commands::command::{Command, CommandMeta};
use crate::commands::lifecycle_cmd::{LifecycleCommand, lifecycle_meta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = lifecycle_meta("start", "Start a package");

pub struct Start;

impl Start {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Start {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Start {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_lifecycle(npm, args)
    }
}

impl LifecycleCommand for Start {}
