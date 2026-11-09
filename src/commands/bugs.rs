//! Port of `lib/commands/bugs.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::commands::package_url_cmd::{
    HostedKind, PackageUrlCommand, hosted_url, package_url_meta,
};
use crate::npm::Npm;
use crate::utils::errors::Result;
use serde_json::Value;

pub static META: CommandMeta =
    package_url_meta("bugs", "Report bugs for a package in a web browser");

pub struct Bugs;

impl Bugs {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Bugs {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Bugs {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_url_commands(npm, args)
    }
}

impl PackageUrlCommand for Bugs {
    fn get_url(&self, _spec: &str, mani: &Value) -> Result<String> {
        if let Some(bugs) = mani.get("bugs") {
            match bugs {
                Value::String(s) => return Ok(s.clone()),
                Value::Object(o) => {
                    if let Some(url) = o.get("url").and_then(Value::as_str) {
                        return Ok(url.to_string());
                    }
                    if let Some(email) = o.get("email").and_then(Value::as_str) {
                        return Ok(format!("mailto:{email}"));
                    }
                }
                _ => {}
            }
        }
        if let Some(url) = hosted_url(mani, HostedKind::Bugs) {
            return Ok(url);
        }
        let name = mani.get("name").and_then(Value::as_str).unwrap_or("");
        Ok(format!("https://www.npmjs.com/package/{name}"))
    }
}
