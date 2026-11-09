//! Port of `lib/commands/fund.js`. Reading the funding tree needs the
//! arborist tree, and opening URLs needs the tree or the registry, so only
//! argument validation and the `--which` handling are ported for now.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("fund", "Retrieve funding information")
    .with_params(&["json", "browser", "unicode", "workspace", "which"])
    .with_usage(&["[<package-spec>]"])
    .arborist();

pub struct Fund;

impl Fund {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Fund {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Fund {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let which = npm.store().get_str("which");
        let funding_source_number = if which.is_empty() {
            None
        } else {
            match which.trim().parse::<usize>() {
                Ok(number) if number >= 1 => Some(number),
                _ => {
                    return Err(NpmError::new(
                        "`npm Fund [<package-spec>] --which=fundingSourceNumber` must be given a positive integer",
                    )
                    .with_code("EFUNDNUMBER"));
                }
            }
        };
        if npm.global() {
            return Err(NpmError::new(
                "`npm Fund [<package-spec>]` does not support global packages",
            )
            .with_code("EFUNDGLOBAL"));
        }
        let _ = funding_source_number;
        let spec = args.first().cloned().unwrap_or_default();
        Err(NpmError::new(if spec.is_empty() {
            "Cannot read funding information: loading the install tree is not implemented yet"
        } else {
            "Cannot open a funding URL: resolving packages is not implemented yet"
        })
        .with_code("ENOTSUP"))
    }
}
