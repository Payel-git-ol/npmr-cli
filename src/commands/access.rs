//! Port of `lib/commands/access.js`. Argument validation, package name
//! resolution and output formatting are ported; `libnpmaccess` calls report
//! `ENOTSUP` until the registry client exists.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta =
    CommandMeta::new("access", "Set access level on published packages")
        .with_params(&["json", "otp", "registry"])
        .with_usage(&[
            "list packages [<user>|<scope>|<scope:team>] [<package>]",
            "list collaborators [<package> [<user>]]",
            "get status [<package>]",
            "set status=public|private [<package>]",
            "set mfa=none|publish|automation [<package>]",
            "grant <read-only|read-write> <scope:team> [<package>]",
            "revoke <scope:team> [<package>]",
        ])
        .skip_validation();

const COMMANDS: &[&str] = &["get", "grant", "list", "revoke", "set"];

const SET_COMMANDS: &[&str] = &[
    "status=public",
    "status=private",
    "mfa=none",
    "mfa=publish",
    "mfa=automation",
    "2fa=none",
    "2fa=publish",
    "2fa=automation",
];

pub struct Access;

impl Access {
    pub fn new() -> Self {
        Self
    }

    fn grant(&self, permissions: &str, scope: &str, pkg: Option<&str>) -> Result<()> {
        let _ = (scope, pkg);
        Err(registry_stub(&format!("granting {permissions}")))
    }

    fn revoke(&self, scope: &str, pkg: Option<&str>) -> Result<()> {
        let _ = (scope, pkg);
        Err(registry_stub("revoking access"))
    }

    fn list_packages(&self, npm: &dyn Npm, owner: Option<&str>, pkg: Option<&str>) -> Result<()> {
        let owner = match owner {
            Some(owner) => owner.to_string(),
            None => crate::utils::get_identity::get_identity(npm)?,
        };
        let _ = (owner, pkg);
        Err(registry_stub("listing packages"))
    }

    fn list_collaborators(&self, npm: &dyn Npm, pkg: Option<&str>, user: Option<&str>) -> Result<()> {
        let pkg_name = self.get_package(npm, pkg, false)?;
        let _ = (pkg_name, user);
        Err(registry_stub("listing collaborators"))
    }

    fn get_status(&self, npm: &dyn Npm, pkg: Option<&str>) -> Result<()> {
        let pkg_name = self.get_package(npm, pkg, false)?;
        let _ = pkg_name;
        Err(registry_stub("reading package visibility"))
    }

    fn set(&self, npm: &dyn Npm, subcmd: &str, pkg: Option<&str>) -> Result<()> {
        let (subkey, subval) = subcmd.split_once('=').unwrap_or((subcmd, ""));
        match subkey {
            "mfa" | "2fa" => {
                let pkg_name = self.get_package(npm, pkg, false)?;
                let _ = (pkg_name, subval);
                Err(registry_stub("setting MFA"))
            }
            "status" => {
                let pkg_name = self.get_package(npm, pkg, true)?;
                let _ = (pkg_name, subval);
                Err(registry_stub("setting package access"))
            }
            _ => Err(META.usage_error(format!("set {subcmd} is not a valid access command"))),
        }
    }

    fn get_package(&self, npm: &dyn Npm, name: Option<&str>, require_scope: bool) -> Result<String> {
        let name = match name {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => {
                let manifest = std::fs::read_to_string(npm.prefix().join("package.json")).ok();
                let Some(manifest) = manifest else {
                    return Err(NpmError::new(
                        "no package name given and no package.json found",
                    )
                    .with_code("ENOENT"));
                };
                let content: Value = serde_json::from_str(&manifest)?;
                content
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            }
        };
        if require_scope && !name.starts_with('@') {
            return Err(META.usage_error(
                "This command is only available for scoped packages.",
            ));
        }
        Ok(name)
    }

    /// Prints the `item: value` table, honouring `--json`.
    fn output(&self, npm: &dyn Npm, items: &BTreeMap<String, String>, limiter: Option<&str>) {
        let mut outputs = BTreeMap::new();
        for (item, value) in items {
            let mapped = match value.as_str() {
                "read" => "read-only",
                "write" => "read-write",
                other => other,
            };
            outputs.insert(item.clone(), mapped.to_string());
        }
        if npm.json() {
            let json = serde_json::to_string_pretty(&outputs).unwrap_or_default();
            crate::output::standard(json);
            return;
        }
        for (item, value) in outputs {
            if limiter.is_none_or(|limit| limit == item) {
                crate::output::standard(format!("{item}: {value}"));
            }
        }
    }
}

impl Default for Access {
    fn default() -> Self {
        Self::new()
    }
}

fn registry_stub(action: &str) -> NpmError {
    NpmError::new(format!(
        "Cannot {action}: registry access is not implemented yet"
    ))
    .with_code("ENOTSUP")
}

impl Command for Access {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        if cmd.is_empty() {
            return Err(META.usage_error(""));
        }
        if !COMMANDS.contains(&cmd) {
            return Err(META.usage_error(format!("{cmd} is not a valid access command")));
        }
        let Some(subcmd) = args.get(1).map(String::as_str).filter(|s| !s.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let rest = &args[2..];
        let arg = |i: usize| rest.get(i).map(String::as_str).filter(|s| !s.is_empty());
        match cmd {
            "grant" => {
                if !matches!(subcmd, "read-only" | "read-write") {
                    return Err(META.usage_error(
                        "grant must be either `read-only` or `read-write`",
                    ));
                }
                let Some(scope) = arg(0) else {
                    return Err(META.usage_error("`<scope:team>` argument is required"));
                };
                self.grant(subcmd, scope, arg(1))
            }
            "revoke" => self.revoke(subcmd, arg(0)),
            "list" | "ls" => match subcmd {
                "packages" => self.list_packages(&*npm, arg(0), arg(1)),
                "collaborators" => self.list_collaborators(&*npm, arg(0), arg(1)),
                _ => Err(META.usage_error(format!(
                    "list {subcmd} is not a valid access command"
                ))),
            },
            "get" => {
                if subcmd != "status" {
                    return Err(META.usage_error(format!(
                        "get {subcmd} is not a valid access command"
                    )));
                }
                self.get_status(&*npm, arg(0))
            }
            "set" => {
                if !SET_COMMANDS.contains(&subcmd) {
                    return Err(META.usage_error(format!(
                        "set {subcmd} is not a valid access command"
                    )));
                }
                self.set(&*npm, subcmd, arg(0))
            }
            _ => Err(META.usage_error("")),
        }
    }
}
