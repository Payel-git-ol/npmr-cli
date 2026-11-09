//! Port of `lib/commands/install-scripts.js`: `approve`, `deny`, `ls` and
//! `prune` subcommands for the `allowScripts` policy.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::allow_scripts_cmd::{Mode, ModeOptions, run_mode};
use crate::utils::errors::Result;

pub static SUBCOMMANDS: &[&str] = &["approve", "deny", "ls", "prune"];

pub static META: CommandMeta = CommandMeta::new(
    "install-scripts",
    "Manage install-script approvals for dependencies",
)
.with_params(&["all", "allow-scripts-pin", "dry-run", "json"])
.with_usage(&[
    "approve <pkg> [<pkg> ...]",
    "approve --all",
    "deny <pkg> [<pkg> ...]",
    "deny --all",
    "ls",
    "prune",
]);

pub struct InstallScripts;

impl InstallScripts {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InstallScripts {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for InstallScripts {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(sub) = args.first() else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        let mode = match sub.as_str() {
            "approve" => Mode::Approve,
            "deny" => Mode::Deny,
            "ls" | "list" => Mode::List,
            "prune" => Mode::Prune,
            other => {
                return Err(META.usage_error(format!("`{other}` is not a recognized subcommand.")));
            }
        };
        run_mode(
            &META,
            npm,
            mode,
            rest,
            ModeOptions {
                pending_flag: false,
            },
        )
    }
}
