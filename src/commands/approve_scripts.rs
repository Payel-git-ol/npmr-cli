//! Port of `lib/commands/approve-scripts.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::allow_scripts_cmd::{Mode, ModeOptions, run_mode};
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new(
    "approve-scripts",
    "Approve install scripts for specific dependencies",
)
.with_params(&["all", "allow-scripts-pending", "allow-scripts-pin", "json"])
.with_usage(&["<pkg> [<pkg> ...]", "--all", "--allow-scripts-pending"]);

pub struct ApproveScripts;

impl ApproveScripts {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ApproveScripts {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for ApproveScripts {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        run_mode(
            &META,
            npm,
            Mode::Approve,
            args,
            ModeOptions {
                pending_flag: true,
            },
        )
    }
}
