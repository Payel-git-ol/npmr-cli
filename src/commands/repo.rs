//! Port of `lib/commands/repo.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::commands::package_url_cmd::{
    HostedKind, PackageUrlCommand, hosted_url, package_url_meta,
};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use serde_json::Value;

pub static META: CommandMeta =
    package_url_meta("repo", "Open package repository page in the browser");

pub struct Repo;

impl Repo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Repo {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Repo {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_url_commands(npm, args)
    }
}

impl PackageUrlCommand for Repo {
    fn get_url(&self, spec: &str, mani: &Value) -> Result<String> {
        let rurl = match mani.get("repository") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => o.get("url").and_then(Value::as_str).map(str::to_string),
            _ => None,
        };
        let Some(rurl) = rurl else {
            return Err(
                NpmError::new("no repository: could not get url".to_string())
                    .with_code("ENOREPO")
                    .with_summary(spec.to_string()),
            );
        };

        if let Some(url) = hosted_url(mani, HostedKind::Browse) {
            return Ok(url);
        }

        unknown_hosted_url(&rurl).ok_or_else(|| {
            NpmError::new("no repository: could not get url")
                .with_code("ENOREPO")
                .with_summary(spec.to_string())
        })
    }
}

/// Normalises an arbitrary git URL to a browsable https URL.
fn unknown_hosted_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let host = parsed.host_str()?;
    let path = parsed.path().trim_end_matches(".git");
    Some(format!("https://{host}{path}"))
}
