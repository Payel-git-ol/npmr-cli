//! Port of `lib/commands/install.js`.

use std::path::Path;

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::{NpmError, Result};
use crate::utils::reify_finish::ArboristRun;

pub static INSTALL_PARAMS: &[&str] = &[
    "save",
    "save-exact",
    "global",
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "omit",
    "include",
    "strict-peer-deps",
    "prefer-dedupe",
    "package-lock",
    "package-lock-only",
    "foreground-scripts",
    "ignore-scripts",
    "allow-directory",
    "allow-file",
    "allow-git",
    "allow-remote",
    "allow-scripts",
    "strict-allow-scripts",
    "dangerously-allow-all-scripts",
    "audit",
    "before",
    "min-release-age",
    "min-release-age-exclude",
    "bin-links",
    "fund",
    "dry-run",
    "cpu",
    "os",
    "libc",
];

pub const INSTALL_USAGE: &[&str] = &["[<package-spec> ...]"];

pub static META: CommandMeta = CommandMeta::new("install", "Install a package")
    .with_params(INSTALL_PARAMS)
    .with_usage(&["[<package-spec> ...]"])
    .arborist();

pub struct Install;

impl Install {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Install {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalises the argument list the same way `Install.exec` does: drops the
/// prefix itself, defaults `-g` without args to `.` and rejects empty specs
/// for global installs.
pub fn install_args(npm: &dyn Npm, args: &[String]) -> Result<Vec<String>> {
    let mut args: Vec<String> = args
        .iter()
        .filter(|arg| {
            let path = crate::utils::errors::resolve_path(&npm.local_prefix().to_path_buf(), arg);
            path != npm.prefix()
        })
        .cloned()
        .collect();
    if npm.global() && args.is_empty() {
        args.push(".".into());
    }
    if args.iter().any(String::is_empty) {
        return Err(META.usage_error(""));
    }
    Ok(args)
}

fn run_root_scripts(npm: &mut (dyn Npm + '_), _path: &Path, events: &[&str]) {
    for event in events {
        let args = vec![(*event).to_string()];
        let _ = npm.exec("run", &args);
    }
}

impl Command for Install {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let is_global = npm.global();
        let where_ = if is_global {
            npm.global_dir().parent().map(Path::to_path_buf).unwrap_or_else(|| npm.prefix())
        } else {
            npm.prefix()
        };
        let ignore_scripts = npm.store().get_bool("ignore-scripts");

        let args = install_args(npm, args)?;

        if is_global && args.iter().any(|a| a == "npm" || a.starts_with("npm@")) {
            crate::utils::log::warn(
                "install",
                &[&"Global installs of npm are not supported yet; the requested version was not verified."],
            );
        }

        let run_root_lifecycle = args.is_empty() && !is_global && !ignore_scripts;
        if run_root_lifecycle {
            run_root_scripts(npm, &where_, &["preinstall"]);
        }

        ArboristRun {
            prefix: where_.clone(),
            add: &args,
            remove: &[],
            update: &[],
            save: None,
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)?;

        if run_root_lifecycle {
            run_root_scripts(
                npm,
                &where_,
                &[
                    "install",
                    "postinstall",
                    "prepublish",
                    "preprepare",
                    "prepare",
                    "postprepare",
                ],
            );
        }
        Ok(())
    }
}

impl ArboristCommand for Install {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}

/// Shared by `ci`, whose argument handling is stricter.
pub fn global_error(command: &str) -> NpmError {
    NpmError::new(format!("`npm {command}` does not work for global packages")).with_code("ECIGLOBAL")
}

/// Whether `--audit` was explicitly requested.
pub fn audit_requested(npm: &dyn Npm) -> bool {
    npm.store().find("audit") == Some(ConfigSource::Cli)
}
