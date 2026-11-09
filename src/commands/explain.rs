//! Port of `lib/commands/explain.js`. Finding the matching nodes needs the
//! arborist tree, so the command validates its arguments and reports
//! `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("explain", "Explain installed packages")
    .with_params(&["json", "workspace"])
    .with_usage(&["<package-spec>"])
    .arborist();

pub struct Explain;

impl Explain {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Explain {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Explain {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(META.usage_error(""));
        }
        let _ = npm;
        Err(NpmError::new(format!(
            "Cannot explain {}: loading the install tree is not implemented yet",
            args.join(", ")
        ))
        .with_code("ENOTSUP"))
    }
}
