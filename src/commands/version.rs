//! Port of `lib/commands/version.js`. Listing versions is ported; bumping
//! needs `libnpmversion` (manifest rewrite, git commit/tag, lifecycle scripts)
//! and reports `ENOTSUP`.

use std::path::Path;

use serde_json::{Map, Value};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("version", "Bump a package version")
    .with_params(&[
        "allow-same-version",
        "commit-hooks",
        "git-tag-version",
        "json",
        "preid",
        "sign-git-tag",
        "save",
        "workspace",
        "workspaces",
        "workspaces-update",
        "include-workspace-root",
        "ignore-scripts",
    ])
    .with_usage(&[
        "[<newversion> | major | minor | patch | premajor | preminor | prepatch | prerelease | from-git]",
    ])
    .workspaces()
    .skip_validation();

/// Version keys accepted on the command line.
pub const VERSION_BUMPES: &[&str] = &[
    "major",
    "minor",
    "patch",
    "premajor",
    "preminor",
    "prepatch",
    "prerelease",
    "from-git",
];

fn read_name_version(path: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path.join("package.json")).ok()?;
    let pkg: Value = serde_json::from_str(&text).ok()?;
    let name = pkg.get("name").and_then(Value::as_str)?.to_string();
    let version = pkg.get("version").and_then(Value::as_str)?.to_string();
    Some((name, version))
}

pub struct Version;

impl Version {
    pub fn new() -> Self {
        Self
    }

    fn list(&self, npm: &dyn Npm, mut results: Map<String, Value>) -> Result<()> {
        if let Some((name, version)) = read_name_version(&npm.prefix()) {
            results.insert(name, Value::String(version));
        }
        results.insert("npm".into(), Value::String(npm.version().to_string()));
        results.insert("node".into(), Value::String("rust".into()));
        if npm.json() {
            let json = serde_json::to_string_pretty(&Value::Object(results))?;
            crate::output::standard(json);
        } else {
            for (key, value) in results {
                crate::output::standard(format!("{key}: {}", value.as_str().unwrap_or_default()));
            }
        }
        Ok(())
    }

    fn list_workspaces(&self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let mut results = Map::new();
        for path in workspaces.values() {
            if let Some((name, version)) = read_name_version(Path::new(path)) {
                results.insert(name, Value::String(version));
            }
        }
        self.list(&*npm, results)
    }

    fn change(&self, npm: &dyn Npm, bump: &str) -> Result<()> {
        let _prefix = npm.store().get_str("tag-version-prefix");
        Err(NpmError::new(format!(
            "Cannot bump the version to {bump}: libnpmversion is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Version {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        match args {
            [] => self.list(&*npm, Map::new()),
            [bump] => self.change(&*npm, bump),
            _ => Err(META.usage_error("")),
        }
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        match args {
            [] => self.list_workspaces(npm),
            [bump] => {
                let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
                let prefix = npm.store().get_str("tag-version-prefix");
                for name in workspaces.keys() {
                    crate::output::standard(name);
                    let _ = &prefix;
                    self.change(&*npm, bump)?;
                }
                Ok(())
            }
            _ => Err(META.usage_error("")),
        }
    }
}
