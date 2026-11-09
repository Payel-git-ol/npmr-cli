//! Port of `lib/commands/whoami.js`. `get-identity` needs `GET /-/whoami`
//! against the registry; until the fetch client lands this reports the
//! configured `_authToken`-based username if one is known locally.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta =
    CommandMeta::new("whoami", "Display npm username").with_params(&["registry"]);

pub struct Whoami;

impl Whoami {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Whoami {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Whoami {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        let username = crate::utils::get_identity::get_identity(npm)?;
        npm.display().standard(&username);
        Ok(())
    }
}
