//! Port of `lib/commands/ls.js` (plus the `ll` alias).

use crate::commands::ArboristCommand;
use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::Result;

const LS_PARAMS: &[&str] = &[
    "all",
    "json",
    "long",
    "parseable",
    "global",
    "depth",
    "omit",
    "include",
    "link",
    "package-lock-only",
    "unicode",
    "workspace",
    "workspaces",
    "include-workspace-root",
    "install-links",
];

pub static META: CommandMeta = CommandMeta::new("ls", "List installed packages")
    .with_usage(&["<package-spec>"])
    .with_params(LS_PARAMS)
    .arborist();

pub static LL_META: CommandMeta = CommandMeta::new("ll", "List installed packages")
    .with_usage(&["<package-spec>"])
    .with_params(LS_PARAMS)
    .arborist();

pub struct Ls {
    meta: &'static CommandMeta,
}

impl Ls {
    pub fn new() -> Self {
        Self { meta: &META }
    }
}

impl Default for Ls {
    fn default() -> Self {
        Self::new()
    }
}

/// `npm ll`: `npm ls` with `--long` forced on.
pub struct Ll;

impl Ll {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Ll {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Ls {
    fn meta(&self) -> &'static CommandMeta {
        self.meta
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(self.meta, args)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_with_workspaces(npm, args)
    }
}

impl ArboristCommand for Ls {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        let global = npm.global();
        let json = npm.json();
        let parseable = npm.store().get_bool("parseable");
        let long = npm.store().get_bool("long") || npm.command() == Some("ll");
        let path = if global {
            npm.global_dir()
        } else {
            npm.prefix()
        };

        let tree = load_tree(npm, &path)?;

        if json {
            crate::output::set_meta(true);
            crate::output::standard(
                serde_json::to_string_pretty(&tree).unwrap_or_else(|_| "{}".into()),
            );
            return Ok(());
        }
        let report = crate::utils::installed::format_tree(&tree, parseable, long);
        crate::output::set_meta(false);
        if report.trim().is_empty() {
            return Ok(());
        }
        for line in report.lines() {
            crate::output::standard(line);
        }
        Ok(())
    }
}

impl Command for Ll {
    fn meta(&self) -> &'static CommandMeta {
        &LL_META
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&LL_META, args)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let mut ls = Ls::new();
        ls.meta = &LL_META;
        ls.exec(npm, args)
    }
}

/// Builds the installed tree. TODO: use arborist once it is ported; for now
/// only the root package and its direct dependencies from `node_modules` are
/// reported.
fn load_tree(npm: &dyn Npm, path: &std::path::Path) -> Result<serde_json::Value> {
    crate::utils::installed::read_tree(npm, path)
}
