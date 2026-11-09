//! Port of `lib/commands/audit.js`. Both subcommands need the arborist tree
//! (and a registry audit endpoint), so they validate their arguments and then
//! report `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::verbose;
use crate::utils::resolve_allow_scripts::resolve_allow_scripts;

pub static META: CommandMeta = CommandMeta::new("audit", "Run a security audit")
    .with_params(&[
        "audit-level",
        "dry-run",
        "force",
        "json",
        "package-lock-only",
        "package-lock",
        "omit",
        "include",
        "foreground-scripts",
        "ignore-scripts",
        "include-attestations",
        "workspace",
        "workspaces",
        "include-workspace-root",
    ])
    .with_usage(&["[fix|signatures]"])
    .arborist();

pub struct Audit;

impl Audit {
    pub fn new() -> Self {
        Self
    }

    fn audit_advisories(&self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let fix = args.first().map(String::as_str) == Some("fix");
        if !npm.store().get_bool("package-lock") && fix {
            return Err(META.usage_error("fix cannot be used without a package-lock"));
        }
        let _reporter = if npm.json() { "json" } else { "detail" };
        let _policy = resolve_allow_scripts(&*npm, false)?;
        Err(NpmError::new(if fix {
            "npm audit fix requires the arborist tree, which is not implemented yet"
        } else {
            "npm audit requires the arborist tree and a registry audit endpoint, which are not implemented yet"
        })
        .with_code("ENOTSUP"))
    }

    fn audit_signatures(&self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        if npm.global() {
            return Err(
                NpmError::new("`npm audit signatures` does not support global packages")
                    .with_code("EAUDITGLOBAL"),
            );
        }
        verbose("audit", &[&"loading installed dependencies"]);
        Err(NpmError::new(
            "npm audit signatures requires the arborist tree, which is not implemented yet",
        )
        .with_code("ENOTSUP"))
    }
}

impl Default for Audit {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Audit {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.first().map(String::as_str) == Some("signatures") {
            self.audit_signatures(npm)
        } else {
            self.audit_advisories(npm, args)
        }
    }
}
