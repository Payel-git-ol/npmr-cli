//! Port of `lib/arborist-cmd.js`. Shared base for commands that build or
//! mutate the `node_modules` tree: defaults `--audit`, resolves workspaces and
//! hands off to the (future) Arborist reify pipeline.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::Result;
use crate::utils::get_workspaces;

/// Trait implemented by commands that reify a tree.
pub trait ArboristCommand: Command {
    /// Runs the actual arborist work for the resolved workspace set.
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()>;

    /// Resolves workspaces then delegates to [`exec_arborist`].
    fn exec_with_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.apply_defaults(npm);
        if npm.store().get_bool("workspaces") || !npm.store().get_list("workspace").is_empty() {
            let workspaces = self.resolve_workspaces(npm)?;
            let mut npm_ctx = WorkspaceContext::new(npm, workspaces);
            return self.exec_arborist(&mut npm_ctx, args);
        }
        self.exec_arborist(npm, args)
    }

    /// Mirrors the constructor logic: turn `audit` off when the command runs
    /// outside a project context, warn when global and audit are both set.
    fn apply_defaults(&self, npm: &mut (dyn Npm + '_)) {
        let store = npm.store_mut();
        let location_project = store.get_str("location") == "project"
            || (store.is_default("location")
                && !store.get_bool("global")
                && store.local_package.is_some());

        if store.is_default("audit") && (store.global() || !location_project) {
            store.set(Value::Bool(false), "audit", ConfigSource::Cli);
        } else if store.global() && store.get_bool("audit") {
            crate::utils::log::warn(
                "config",
                &[&"includes both --global and --audit, which is currently unsupported."],
            );
        }
    }

    fn resolve_workspaces(&self, npm: &(dyn Npm + '_)) -> Result<BTreeMap<String, String>> {
        let include_root = if self.meta().is_arborist_cmd {
            false
        } else {
            npm.store().get_bool("include-workspace-root")
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| npm.local_prefix().to_path_buf());
        let prefix = npm.local_prefix().to_path_buf();
        let prefix_inside_cwd = cwd.starts_with(&prefix) && cwd != prefix;
        let relative_from = if prefix_inside_cwd {
            prefix.clone()
        } else {
            cwd
        };
        get_workspaces::get_workspaces(&get_workspaces_input(
            npm.store().get_list("workspace"),
            prefix,
            include_root,
            relative_from,
        ))
    }
}

fn get_workspaces_input(
    filters: Vec<String>,
    path: std::path::PathBuf,
    include_workspace_root: bool,
    relative_from: std::path::PathBuf,
) -> get_workspaces::GetWorkspacesInput {
    get_workspaces::GetWorkspacesInput {
        filters,
        path: path.to_string_lossy().into_owned(),
        include_workspace_root,
        relative_from: relative_from.to_string_lossy().into_owned(),
    }
}

/// A [`Npm`] view carrying the resolved workspace set, as `BaseCommand`
/// stores it on `this.workspaces`.
pub struct WorkspaceContext<'a> {
    npm: &'a mut (dyn Npm + 'a),
    #[expect(
        dead_code,
        reason = "workspace-aware commands are not wired to arborist yet"
    )]
    workspaces: BTreeMap<String, String>,
}

#[expect(
    dead_code,
    reason = "workspace-aware commands are not wired to arborist yet"
)]
impl<'a> WorkspaceContext<'a> {
    pub fn new(npm: &'a mut (dyn Npm + 'a), workspaces: BTreeMap<String, String>) -> Self {
        Self { npm, workspaces }
    }

    pub fn names(&self) -> Vec<&str> {
        self.workspaces.keys().map(String::as_str).collect()
    }

    pub fn paths(&self) -> Vec<&str> {
        self.workspaces.values().map(String::as_str).collect()
    }
}

impl Npm for WorkspaceContext<'_> {
    fn version(&self) -> &'static str {
        self.npm.version()
    }
    fn flat_options(&self) -> serde_json::Map<String, Value> {
        self.npm.flat_options()
    }
    fn color(&self) -> bool {
        self.npm.color()
    }
    fn log_color(&self) -> bool {
        self.npm.log_color()
    }
    fn global(&self) -> bool {
        self.npm.global()
    }
    fn silent(&self) -> bool {
        self.npm.silent()
    }
    fn lockfile_version(&self) -> u8 {
        self.npm.lockfile_version()
    }
    fn local_prefix(&self) -> &std::path::Path {
        self.npm.local_prefix()
    }
    fn global_prefix(&self) -> &std::path::Path {
        self.npm.global_prefix()
    }
    fn store(&self) -> &crate::store::Store {
        self.npm.store()
    }
    fn store_mut(&mut self) -> &mut crate::store::Store {
        self.npm.store_mut()
    }
    fn display(&self) -> &crate::utils::display::Display {
        self.npm.display()
    }
    fn loaded(&self) -> bool {
        self.npm.loaded()
    }
    fn command(&self) -> Option<&str> {
        self.npm.command()
    }
    fn argv(&self) -> &[String] {
        self.npm.argv()
    }
    fn title(&self) -> &str {
        self.npm.title()
    }
    fn npm_root(&self) -> &std::path::Path {
        self.npm.npm_root()
    }
    fn log_files(&self) -> &[std::path::PathBuf] {
        self.npm.log_files()
    }
    fn child_env(&self) -> &std::collections::BTreeMap<String, String> {
        self.npm.child_env()
    }
    fn exec(&mut self, name: &str, args: &[String]) -> Result<()> {
        self.npm.exec(name, args)
    }
}

/// Shared params for all arborist commands.
#[expect(dead_code, reason = "commands declare their params explicitly for now")]
pub const ARBORIST_PARAMS: &[&str] = &[
    "workspace",
    "workspaces",
    "include-workspace-root",
    "install-links",
];

/// Convenience: builds a `ParsedFlags` for a command with the arborist base
/// metadata plus extra definitions.
#[expect(dead_code, reason = "commands call `parse_flags` directly")]
pub fn parse(meta: &CommandMeta, args: &[String]) -> ParsedFlags {
    crate::commands::parse_flags(meta, args)
}
