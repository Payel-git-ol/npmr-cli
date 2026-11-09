//! Port of `lib/commands/link.js`.

use std::path::{Path, PathBuf};

use crate::commands::arborist_cmd::ArboristCommand;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::Result;
use crate::utils::reify_finish::ArboristRun;

pub static LINK_PARAMS: &[&str] = &[
    "save",
    "save-exact",
    "global",
    "install-strategy",
    "legacy-bundling",
    "global-style",
    "strict-peer-deps",
    "package-lock",
    "omit",
    "include",
    "ignore-scripts",
    "allow-directory",
    "allow-file",
    "allow-git",
    "allow-remote",
    "audit",
    "bin-links",
    "fund",
    "dry-run",
];

pub static META: CommandMeta = CommandMeta::new("link", "Symlink a package folder")
    .with_params(LINK_PARAMS)
    .with_usage(&["[<package-spec>]"])
    .arborist();

pub struct Link;

impl Link {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

fn global_top(npm: &dyn Npm) -> PathBuf {
    npm.global_dir()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| npm.prefix())
}

/// Package name for a `link` positional: directories use their manifest name.
pub fn link_name(prefix: &Path, arg: &str) -> String {
    let path = crate::utils::errors::resolve_path(prefix, arg);
    if path.is_dir() {
        if let Ok(text) = std::fs::read_to_string(path.join("package.json")) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(name) = manifest.get("name").and_then(serde_json::Value::as_str) {
                    return name.to_string();
                }
            }
        }
    }
    match arg.rfind('@') {
        Some(idx) if idx > 0 => arg[..idx].to_string(),
        _ => arg.to_string(),
    }
}

fn save_requested(npm: &dyn Npm) -> bool {
    let store = npm.store();
    (store.find("save") == Some(ConfigSource::Cli) && store.get_bool("save"))
        || store.get_bool("save-optional")
        || store.get_bool("save-peer")
        || store.get_bool("save-dev")
        || store.get_bool("save-prod")
}

impl Command for Link {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let prefix = npm.prefix();
        let args: Vec<String> = args
            .iter()
            .filter(|arg| crate::utils::errors::resolve_path(&prefix, arg) != prefix)
            .cloned()
            .collect();
        if args.is_empty() {
            return self.link_pkg(npm);
        }
        self.link_install(npm, &args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}

impl Link {
    fn link_install(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let global = global_top(npm);
        let names: Vec<String> = args
            .iter()
            .map(|arg| link_name(npm.local_prefix(), arg))
            .collect();
        let adds: Vec<String> = names
            .iter()
            .map(|name| format!("file:{}", global.join("node_modules").join(name).display()))
            .collect();
        ArboristRun {
            prefix: npm.prefix(),
            add: &adds,
            remove: &[],
            update: &[],
            save: Some(save_requested(npm)),
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }

    fn link_pkg(&mut self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let mut paths = vec![npm.prefix()];
        if npm.store().get_bool("workspaces") || !npm.store().get_list_is_empty("workspace") {
            if let Ok(workspaces) = crate::utils::set_workspaces::set_workspaces(&*npm) {
                paths = workspaces.values().map(PathBuf::from).collect();
            }
        }
        let adds: Vec<String> = paths
            .iter()
            .map(|path| format!("file:{}", path.display()))
            .collect();
        ArboristRun {
            prefix: global_top(npm),
            add: &adds,
            remove: &[],
            update: &[],
            save: Some(false),
            dry_run: npm.store().get_bool("dry-run"),
            workspaces: &[],
            package_lock: None,
        }
        .execute(npm)
    }
}

impl ArboristCommand for Link {
    fn exec_arborist(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }
}
