//! Port of `lib/commands/token.js`. Subcommand dispatch and argument
//! validation are ported; the token API calls report `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("token", "Manage your authentication tokens")
    .with_params(&[
        "name",
        "token-description",
        "expires",
        "packages",
        "packages-all",
        "scopes",
        "orgs",
        "packages-and-scopes-permission",
        "orgs-permission",
        "cidr",
        "bypass-2fa",
        "password",
        "registry",
        "otp",
        "read-only",
    ])
    .with_usage(&["list", "revoke <id|token>", "create"])
    .skip_validation();

pub struct Token;

impl Token {
    pub fn new() -> Self {
        Self
    }

    fn list(&self) -> Result<()> {
        Err(registry_stub("listing tokens"))
    }

    fn rm(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(META.usage_error("`<tokenKey>` argument is required."));
        }
        Err(registry_stub("revoking tokens"))
    }
}

impl Default for Token {
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

impl Command for Token {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(sub) = args.first() else {
            return self.list();
        };
        match sub.as_str() {
            "list" | "ls" => self.list(),
            "rm" | "delete" | "revoke" | "remove" => self.rm(&args[1..]),
            "create" => Err(registry_stub("creating tokens")),
            other => Err(META.usage_error(format!("{other} is not a recognized subcommand."))),
        }
    }
}
