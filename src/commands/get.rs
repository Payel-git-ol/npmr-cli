//! Port of `lib/commands/get.js`: delegates to `npm config get`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("get", "Get a value from the npm configuration")
    .with_usage(&["[<key> ...] (See `npm config`)"])
    .with_params(&["long"]);

pub struct Get;

impl Get {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Get {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Get {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let mut config_args = vec!["get".to_string()];
        config_args.extend(args.iter().cloned());
        npm.exec("config", &config_args)
    }
}
