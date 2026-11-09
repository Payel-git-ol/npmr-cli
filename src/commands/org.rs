//! Port of `lib/commands/org.js`. Argument validation is ported; `libnpmorg`
//! calls report `ENOTSUP` until the registry client exists.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("org", "Manage orgs")
    .with_params(&["registry", "otp", "json", "parseable"])
    .with_usage(&[
        "set orgname username [developer | admin | owner]",
        "rm orgname username",
        "ls orgname [<username>]",
    ])
    .skip_validation();

const ROLES: &[&str] = &["owner", "admin", "developer"];

pub struct Org;

impl Org {
    pub fn new() -> Self {
        Self
    }

    fn set(&self, orgname: Option<&str>, username: Option<&str>, role: Option<&str>) -> Result<()> {
        let Some(orgname) = orgname.filter(|s| !s.is_empty()) else {
            return Err(NpmError::new("First argument `orgname` is required."));
        };
        let Some(username) = username.filter(|s| !s.is_empty()) else {
            return Err(NpmError::new("Second argument `username` is required."));
        };
        let role = role.filter(|s| !s.is_empty()).unwrap_or("developer");
        if !ROLES.contains(&role) {
            return Err(NpmError::new(
                "Third argument `role` must be one of `owner`, `admin`, or `developer`, with `developer` being the default value if omitted.",
            ));
        }
        let _ = (orgname, username, role);
        Err(registry_stub("adding a member"))
    }

    fn rm(&self, orgname: Option<&str>, username: Option<&str>) -> Result<()> {
        let Some(_orgname) = orgname.filter(|s| !s.is_empty()) else {
            return Err(NpmError::new("First argument `orgname` is required."));
        };
        let Some(_username) = username.filter(|s| !s.is_empty()) else {
            return Err(NpmError::new("Second argument `username` is required."));
        };
        Err(registry_stub("removing a member"))
    }

    fn ls(&self, orgname: Option<&str>, _username: Option<&str>) -> Result<()> {
        let Some(_orgname) = orgname.filter(|s| !s.is_empty()) else {
            return Err(NpmError::new("First argument `orgname` is required."));
        };
        Err(registry_stub("listing organization members"))
    }
}

impl Default for Org {
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

impl Command for Org {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        let orgname = args.get(1).map(String::as_str);
        let username = args.get(2).map(String::as_str);
        let role = args.get(3).map(String::as_str);
        match cmd {
            "add" | "set" => self.set(orgname, username, role),
            "rm" => self.rm(orgname, username),
            "ls" => self.ls(orgname, username),
            _ => Err(META.usage_error("")),
        }
    }
}
