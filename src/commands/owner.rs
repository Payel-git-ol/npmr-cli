//! Port of `lib/commands/owner.js`. Package name resolution and the
//! workspace selection rules are ported; registry calls report `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;

pub static META: CommandMeta = CommandMeta::new("owner", "Manage package owners")
    .with_params(&["registry", "otp", "workspace", "workspaces"])
    .with_usage(&[
        "add <user> <package-spec>",
        "rm <user> <package-spec>",
        "ls <package-spec>",
    ])
    .workspaces();

pub struct Owner;

impl Owner {
    pub fn new() -> Self {
        Self
    }

    fn package_name(npm: &dyn Npm, pkg: Option<&str>) -> Result<String> {
        if let Some(pkg) = pkg.filter(|p| !p.is_empty()) {
            return Ok(pkg.to_string());
        }
        if npm.global() {
            return Err(META.usage_error(""));
        }
        let manifest = std::fs::read_to_string(npm.prefix().join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .unwrap_or_default();
        match manifest.get("name").and_then(serde_json::Value::as_str) {
            Some(name) if !name.is_empty() => Ok(name.to_string()),
            _ => Err(META.usage_error("")),
        }
    }

    fn ls(&self, npm: &dyn Npm, pkg: Option<&str>) -> Result<()> {
        let pkg = Self::package_name(npm, pkg)?;
        crate::utils::log::error("owner ls", &[&"Couldn't get owner data", &pkg]);
        Err(NpmError::new(format!(
            "Cannot read owners of {pkg}: registry access is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }

    fn change_owners(&self, npm: &dyn Npm, user: Option<&str>, pkg: Option<&str>) -> Result<()> {
        let Some(_user) = user.filter(|u| !u.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let pkg = Self::package_name(npm, pkg)?;
        Err(NpmError::new(format!(
            "Cannot change owners of {pkg}: registry access is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }

    fn dispatch(&self, npm: &mut (dyn Npm + '_), action: &str, rest: &[String]) -> Result<()> {
        match action {
            "ls" | "list" => self.ls(&*npm, rest.first().map(String::as_str)),
            "add" => self.change_owners(
                &*npm,
                rest.first().map(String::as_str),
                rest.get(1).map(String::as_str),
            ),
            "rm" | "remove" => self.change_owners(
                &*npm,
                rest.first().map(String::as_str),
                rest.get(1).map(String::as_str),
            ),
            _ => Err(META.usage_error("")),
        }
    }
}

impl Default for Owner {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Owner {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let action = args.first().map(String::as_str).unwrap_or("");
        self.dispatch(npm, action, &args[1..])
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let action = args.first().map(String::as_str).unwrap_or("");
        let rest = &args[1..];
        if (action == "ls" && !rest.is_empty()) || rest.len() > 1 {
            let explicit = npm
                .flat_options()
                .get("workspace")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|w| !w.is_empty());
            if !explicit {
                warn("owner", &[&"Ignoring specified workspace(s)"]);
            }
            return self.dispatch(npm, action, rest);
        }
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        for name in workspaces.keys() {
            match action {
                "ls" | "list" => self.ls(&*npm, Some(name))?,
                "add" | "rm" | "remove" => {
                    self.change_owners(&*npm, rest.first().map(String::as_str), Some(name))?
                }
                _ => return Err(META.usage_error("")),
            }
        }
        Ok(())
    }
}
