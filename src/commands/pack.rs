//! Port of `lib/commands/pack.js`. Creating the tarball needs `libnpmpack`
//! (file selection, ignore rules, lifecycle scripts), which is not ported yet,
//! so the command validates its arguments and reports `ENOTSUP`; tarball
//! reporting lives in [`crate::utils::tar`].

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;

pub static META: CommandMeta = CommandMeta::new("pack", "Create a tarball from a package")
    .with_params(&[
        "dry-run",
        "json",
        "pack-destination",
        "workspace",
        "workspaces",
        "include-workspace-root",
        "ignore-scripts",
    ])
    .with_usage(&["<package-spec>"])
    .workspaces();

pub struct Pack;

impl Pack {
    pub fn new() -> Self {
        Self
    }

    fn pack_args(&self, npm: &dyn Npm, args: &[String]) -> Result<()> {
        let args: Vec<String> = if args.is_empty() {
            vec![".".to_string()]
        } else {
            args.iter().filter(|a| !a.is_empty()).cloned().collect()
        };
        let _unicode = npm.store().get_bool("unicode");
        Err(NpmError::new(format!(
            "Cannot pack {}: creating tarballs is not implemented yet",
            args.join(" ")
        ))
        .with_code("ENOTSUP"))
    }
}

impl Default for Pack {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Pack {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.pack_args(&*npm, args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let use_workspaces = args.is_empty() || args.iter().any(|a| a == ".");
        if !use_workspaces {
            warn("pack", &[&"Ignoring workspaces for specified package(s)"]);
            return self.exec(npm, args);
        }
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let mut targets: Vec<String> = workspaces.values().cloned().collect();
        targets.extend(args.iter().filter(|a| *a != ".").cloned());
        self.pack_args(&*npm, &targets)
    }
}
