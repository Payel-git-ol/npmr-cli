//! Port of `lib/commands/trust/index.js` and its subcommands.

pub mod circleci;
pub mod command;
pub mod github;
pub mod gitlab;
mod list;
mod revoke;

pub use command::TrustProvider;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new(
    "trust",
    "Create a trusted relationship between a package and a OIDC provider",
)
.with_usage(&[]);

pub const SUBCOMMANDS: &[&str] = &["github", "gitlab", "circleci", "list", "revoke"];

pub struct Trust;

impl Trust {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Trust {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Trust {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(sub) = args.first() else {
            return Err(self.meta().usage_error(""));
        };
        let rest = &args[1..];
        match sub.as_str() {
            "github" => github::TrustGitHub::new().exec(npm, rest),
            "gitlab" => gitlab::TrustGitLab::new().exec(npm, rest),
            "circleci" => circleci::TrustCircleCi::new().exec(npm, rest),
            "list" => list::TrustList::new().exec(npm, rest),
            "revoke" => revoke::TrustRevoke::new().exec(npm, rest),
            other => Err(NpmError::new(format!("Unknown subcommand: {other}"))
                .with_code("EUNKNOWNCOMMAND")),
        }
    }
}
