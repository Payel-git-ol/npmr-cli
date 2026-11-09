//! Port of `lib/commands/login.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::log::info;

pub static META: CommandMeta = CommandMeta::new("login", "Login to a registry user account")
    .with_usage(&["[--scope=<scope>] [--auth-type=legacy|web]"])
    .with_params(&["registry", "scope", "auth-type"]);

pub struct Login;

impl Login {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Login {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Login {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        let registry = npm.store().get_str("registry");
        let scope = npm.store().get("scope").and_then(|v| v.as_str()).map(str::to_string);

        let username = crate::utils::read_user_info::username("")?;
        let password = crate::utils::read_user_info::password("")?;
        let creds = crate::utils::auth::login(npm, &username, &password)?;

        let host = crate::utils::auth::registry_host(&registry);
        npm.store_mut().set(
            serde_json::Value::String(creds.token),
            &format!("//{host}/:_authToken"),
            crate::store::ConfigSource::User,
        );
        info("login", &[&creds.message]);
        let _ = scope;
        Ok(())
    }
}
