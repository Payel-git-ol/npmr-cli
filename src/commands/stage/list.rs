//! Port of `lib/commands/stage/list.js`.

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::key_values::log_stage_item;

pub static META: CommandMeta = CommandMeta::new("list", "List all staged package versions")
    .with_usage(&["[<package-spec>]"])
    .with_params(&["json", "registry"]);

const PER_PAGE: usize = 100;

pub struct StageList;

impl StageList {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StageList {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let package_filter = match args.first() {
            Some(arg) => {
                let (name, spec) = split_spec(arg);
                if spec != "*" {
                    return Err(
                        self.meta()
                            .usage_error("Version specifiers are not supported for listing staged packages"),
                    );
                }
                Some(name)
            }
            None => None,
        };
        let json = npm.json();

        let all_items = fetch_all_pages(npm, package_filter.as_deref())?;

        if json {
            let value = serde_json::to_string_pretty(&all_items).unwrap_or_default();
            crate::output::set_meta(true);
            crate::output::standard(value);
            return Ok(());
        }

        if all_items.is_empty() {
            match package_filter {
                Some(name) => crate::output::standard(format!(
                    "No staged versions of package name \"{name}\"."
                )),
                None => crate::output::standard("No staged packages found."),
            }
            return Ok(());
        }

        for (i, item) in all_items.iter().enumerate() {
            if i > 0 {
                crate::output::set_meta(false);
                crate::output::standard("");
            }
            crate::output::set_meta(true);
            crate::output::standard(log_stage_item(item));
        }
        Ok(())
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}

/// Splits `name@spec` into `(name, spec)`; a bare name yields `*`.
fn split_spec(arg: &str) -> (String, String) {
    let at = arg.rfind('@').filter(|&i| i > 0);
    match at {
        Some(i) => (arg[..i].to_string(), arg[i + 1..].to_string()),
        None => (arg.to_string(), "*".to_string()),
    }
}

/// TODO: `GET /-/stage?page=<n>&perPage=<n>` until the pages run out.
fn fetch_all_pages(_npm: &dyn Npm, _package_filter: Option<&str>) -> Result<Vec<Value>> {
    Err(NpmError::new(
        "Could not list staged packages: registry request is not implemented yet",
    )
    .with_code("ENOTSUP"))
}
