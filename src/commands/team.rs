//! Port of `lib/commands/team.js`. Argument validation and the `ls`
//! user-vs-team switch are ported; `libnpmteam` calls report `ENOTSUP` until
//! the registry client exists.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new(
    "team",
    "Manage organization teams and team memberships",
)
.with_params(&["registry", "otp", "parseable", "json"])
.with_usage(&[
    "create <scope:team> [--otp <otpcode>]",
    "destroy <scope:team> [--otp <otpcode>]",
    "add <scope:team> <user> [--otp <otpcode>]",
    "rm <scope:team> <user> [--otp <otpcode>]",
    "ls <scope>|<scope:team>",
])
.skip_validation();

pub struct Team;

impl Team {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Team {
    fn default() -> Self {
        Self::new()
    }
}

fn registry_stub(action: &str) -> NpmError {
    NpmError::new(format!(
        "Cannot {action}: registry access is not implemented yet"
    ))
    .with_code("ENOTSUP")
}

impl Command for Team {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        let entity = args
            .get(1)
            .map(|e| e.trim_start_matches('@').to_string())
            .unwrap_or_default();
        match cmd {
            "create" => Err(registry_stub("creating a team")),
            "destroy" => Err(registry_stub("destroying a team")),
            "add" | "rm" => {
                let Some(_user) = args.get(2).filter(|u| !u.is_empty()) else {
                    return Err(META.usage_error(""));
                };
                Err(registry_stub(if cmd == "add" {
                    "adding a team member"
                } else {
                    "removing a team member"
                }))
            }
            "ls" => {
                // Entities are `<scope>:<team>`; without a team we list teams.
                match entity.split_once(':') {
                    Some(_team) => Err(registry_stub("listing team members")),
                    None => Err(registry_stub("listing teams")),
                }
            }
            _ => Err(META.usage_error("")),
        }
    }
}
