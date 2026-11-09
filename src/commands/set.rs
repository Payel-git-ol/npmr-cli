//! Port of `lib/commands/set.js`: delegates to `npm config set`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("set", "Set a value in the npm configuration")
    .with_usage(&["<key>=<value> [<key>=<value> ...] (See `npm config`)"])
    .with_params(&["global", "location"]);

pub struct Set;

impl Set {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Set {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Set {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(self.meta().usage_error(""));
        }
        let mut config_args = vec!["set".to_string()];
        config_args.extend(args.iter().cloned());
        npm.exec("config", &config_args)
    }
}
