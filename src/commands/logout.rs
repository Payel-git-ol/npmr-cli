//! Port of `lib/commands/logout.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::auth::registry_host;
use crate::utils::errors::Result;
use crate::utils::log::notice;

pub static META: CommandMeta = CommandMeta::new("logout", "Log out of the registry")
    .with_usage(&["[--scope=<scope>]"])
    .with_params(&["registry", "scope"]);

pub struct Logout;

impl Logout {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Logout {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Logout {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        let registry = npm.store().get_str("registry");
        let host = registry_host(&registry);
        let token_key = format!("//{host}/:_authToken");
        let has_token = npm
            .store()
            .get(&token_key)
            .and_then(|v| v.as_str())
            .is_some_and(|v| !v.is_empty());

        if !has_token {
            return Err(
                crate::utils::errors::NpmError::new(format!("not logged in to {registry}, so can't log out!"))
                    .with_code("ENEEDAUTH"),
            );
        }

        // TODO: call `DELETE /-/user/token/<token>`; the local token is the
        // authoritative copy for now.
        npm.store_mut()
            .set(serde_json::Value::Null, &token_key, crate::store::ConfigSource::User);
        notice("", &[&format!("logged out of {registry}")]);
        Ok(())
    }
}
