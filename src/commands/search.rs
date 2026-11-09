//! Port of `lib/commands/search.js`. Querying the registry is not ported yet,
//! so a search always comes back empty; output formatting is shared with the
//! future streaming implementation via [`format_search_stream`].

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("search", "Search for packages")
    .with_params(&[
        "json",
        "color",
        "parseable",
        "description",
        "searchlimit",
        "searchopts",
        "searchexclude",
        "registry",
        "prefer-online",
        "prefer-offline",
        "offline",
    ])
    .with_usage(&["<search term> [<search term> ...]"])
    .skip_validation();

pub struct Search;

impl Search {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Search {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let include: Vec<String> = args
            .iter()
            .map(|a| a.to_lowercase())
            .filter(|a| !a.is_empty())
            .collect();
        if include.is_empty() {
            return Err(NpmError::new("search must be called with arguments"));
        }
        crate::utils::log::silly("search", &[&"searching packages"]);

        if npm.json() || npm.store().get_bool("parseable") {
            crate::output::standard("[]");
        } else {
            let terms = args
                .iter()
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(" ");
            crate::output::standard(format!("No matches found for {terms}"));
        }
        crate::utils::log::silly("search", &[&"search completed"]);
        Ok(())
    }
}
