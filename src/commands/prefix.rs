//! Port of `lib/commands/prefix.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("prefix", "Display prefix").with_params(&["global"]);

pub struct Prefix;

impl Prefix {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Prefix {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Prefix {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        npm.display()
            .standard(&npm.prefix().to_string_lossy());
        Ok(())
    }
}
