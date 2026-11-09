//! Port of `lib/commands/explore.js`: runs a command inside an installed
//! package's folder, or opens a shell there.

use std::path::PathBuf;

use crate::commands::command::{Command, CommandMeta};
use crate::commands::run::{run_shell, RunScript};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("explore", "Browse an installed package")
    .with_params(&["shell"])
    .with_usage(&["<pkg> [ -- <command>]"]);

pub struct Explore;

impl Explore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Explore {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Explore {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(pkg_name) = args.first().filter(|a| !a.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let dir = npm.dir();
        let inside = pkg_name.trim_start_matches(['/', '\\']);
        let path: PathBuf = dir.join(inside);
        if !path.starts_with(&dir) || path == dir {
            return Err(META.usage_error(""));
        }

        if !path.join("package.json").is_file() {
            crate::utils::log::error(
                "explore",
                &[&format!("It doesn't look like {pkg_name} is installed.")],
            );
            return Err(NpmError::new(format!("{pkg_name} is not installed")).with_code("ENOENT"));
        }
        let manifest = RunScript::read_package(&path);
        let _ = manifest;

        let rest = &args[1..];
        let command = if rest.is_empty() {
            crate::output::standard(format!(
                "\nExploring {}\nType 'exit' or ^D when finished\n",
                path.display()
            ));
            npm.store().get_str("shell")
        } else {
            rest.join(" ")
        };
        if command.is_empty() {
            return Ok(());
        }
        run_shell(npm, &path, &command)
    }
}
