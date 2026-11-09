//! Port of `lib/commands/root.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta =
    CommandMeta::new("root", "Display npm root").with_params(&["global"]);

pub struct Root;

impl Root {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Root {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        npm.display().standard(&npm.dir().to_string_lossy());
        Ok(())
    }
}
