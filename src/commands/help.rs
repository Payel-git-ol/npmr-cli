//! Port of `lib/commands/help.js`.

use std::path::PathBuf;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::open_url::open_url;

pub static META: CommandMeta = CommandMeta::new("help", "Get help on npm")
    .with_usage(&["<term> [<terms..>]"])
    .with_params(&["viewer"])
    .skip_validation();

pub struct Help;

impl Help {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Help {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Help {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        match args.first().map(String::as_str) {
            Some("search") => npm.exec("help-search", &args[1..]),
            None => {
                let usage = crate::utils::npm_usage::usage(npm);
                crate::output::set_meta(false);
                crate::output::standard(usage);
                Ok(())
            }
            Some(topic) => {
                let man = man_page_path(npm, topic);
                let Some(man) = man else {
                    crate::utils::log::warn("help", &[&format!("No help topic for '{topic}'")]);
                    return Ok(());
                };
                // TODO: run the configured viewer; opening the man page URL is
                // the closest fallback for now.
                open_url(
                    npm,
                    &format!(
                        "https://docs.npmjs.com/cli/v{}/using-npm/config",
                        crate::VERSION.split('.').next().unwrap_or("11")
                    ),
                    &format!("help for {}", man.display()),
                    true,
                )
            }
        }
    }
}

/// `man/man<n>/npm-<topic>.<n>` under the npm root.
fn man_page_path(npm: &dyn Npm, topic: &str) -> Option<PathBuf> {
    let root = npm.npm_root();
    for n in 1..=9 {
        let dir = root.join(format!("man/man{n}"));
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == format!("npm-{topic}.{n}")
                || (!topic.contains('-') && name == format!("npm-{topic}-{n}.{n}"))
            {
                return Some(entry.path());
            }
        }
    }
    None
}
