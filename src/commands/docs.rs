//! Port of `lib/commands/docs.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::commands::package_url_cmd::{HostedKind, PackageUrlCommand, hosted_url, package_url_meta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use serde_json::Value;

pub static META: CommandMeta = package_url_meta(
    "docs",
    "Open documentation for a package in a web browser",
);

pub struct Docs;

impl Docs {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Docs {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Docs {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_url_commands(npm, args)
    }
}

impl PackageUrlCommand for Docs {
    fn get_url(&self, _spec: &str, mani: &Value) -> Result<String> {
        if let Some(homepage) = mani.get("homepage").and_then(Value::as_str) {
            return Ok(homepage.to_string());
        }
        if let Some(url) = hosted_url(mani, HostedKind::Docs) {
            return Ok(url);
        }
        let name = mani.get("name").and_then(Value::as_str).unwrap_or("");
        Ok(format!("https://www.npmjs.com/package/{name}"))
    }
}
