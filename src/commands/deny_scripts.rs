//! Port of `lib/commands/deny-scripts.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::allow_scripts_cmd::{Mode, ModeOptions, run_mode};
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new(
    "deny-scripts",
    "Deny install scripts for specific dependencies",
)
.with_params(&["all", "allow-scripts-pending", "allow-scripts-pin", "json"])
.with_usage(&["<pkg> [<pkg> ...]", "--all"]);

pub struct DenyScripts;

impl DenyScripts {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DenyScripts {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for DenyScripts {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        run_mode(
            &META,
            npm,
            Mode::Deny,
            args,
            ModeOptions {
                pending_flag: true,
            },
        )
    }
}
