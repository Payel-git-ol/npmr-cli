//! Port of `lib/commands/exec.js`. Executing a binary from a local or remote
//! package is `libnpmexec`'s job; until it is ported, `npm exec` reports
//! `ENOTSUP`, but argument validation and workspace iteration match upstream.

use std::path::PathBuf;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::resolve_allow_scripts::resolve_allow_scripts;

pub static META: CommandMeta =
    CommandMeta::new("exec", "Run a command from a local or remote npm package")
        .with_params(&[
            "package",
            "call",
            "workspace",
            "workspaces",
            "include-workspace-root",
            "allow-scripts",
            "strict-allow-scripts",
            "dangerously-allow-all-scripts",
        ])
        .with_usage(&[
            "-- <pkg>[@<version>] [args...]",
            "--package=<pkg>[@<version>] -- <cmd> [args...]",
            "-c '<cmd> [args...]'",
            "--package=foo -c '<cmd> [args...]'",
        ])
        .workspaces()
        .shellout();

pub struct Exec;

impl Exec {
    pub fn new() -> Self {
        Self
    }

    fn call_exec(
        &self,
        npm: &mut (dyn Npm + '_),
        args: &[String],
        run_path: Option<&str>,
    ) -> Result<()> {
        let call = npm.store().get_str("call");
        if !call.is_empty() && !args.is_empty() {
            return Err(META.usage_error(""));
        }

        let _policy = resolve_allow_scripts(&*npm, true)?;

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let _run_path = run_path.map(PathBuf::from).unwrap_or(cwd);

        Err(NpmError::new(
            "npm exec is not implemented yet: running binaries from packages requires libnpmexec",
        )
        .with_code("ENOTSUP"))
    }
}

impl Default for Exec {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Exec {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.call_exec(npm, args, None)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        for (name, path) in workspaces {
            crate::utils::log::info("exec", &[&format!("in workspace {name}"), &path]);
            self.call_exec(npm, args, Some(&path))?;
        }
        Ok(())
    }
}
