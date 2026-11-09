//! Port of `lib/package-url-cmd.js`. Shared by bugs/docs/repo: resolves each
//! spec to a manifest then opens the relevant URL.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use serde_json::Value;

pub trait PackageUrlCommand: Command {
    /// Computes the URL to open for `spec` given its manifest.
    fn get_url(&self, spec: &str, mani: &Value) -> Result<String>;

    /// Manifest lookup. TODO: replace with `pacote.manifest` once the
    /// registry client exists; for local paths read `package.json` directly.
    fn manifest(&self, npm: &(dyn Npm + '_), spec: &str) -> Result<Value> {
        let path = npm.local_prefix().join(spec).join("package.json");
        let content = std::fs::read_to_string(&path).map_err(|err| {
            crate::utils::errors::NpmError::new(format!(
                "could not read manifest for {spec}: {err}"
            ))
            .with_code("E404")
        })?;
        serde_json::from_str(&content).map_err(|err| {
            crate::utils::errors::NpmError::new(format!("could not parse {spec}: {err}"))
                .with_code("EJSONPARSE")
        })
    }

    fn exec_url_commands(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let targets: Vec<String> = if args.is_empty() {
            vec![".".to_string()]
        } else {
            args.to_vec()
        };
        for arg in &targets {
            let mani = self.manifest(npm, arg)?;
            let url = self.get_url(arg, &mani)?;
            crate::utils::log::silly(self.meta().name, &[&"url", &url]);
            let name = mani
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(arg);
            let title = format!("{name} {} available at the following URL", self.meta().name);
            crate::utils::open_url::open_url(npm, &url, &title, false)?;
        }
        Ok(())
    }
}

pub const fn package_url_meta(name: &'static str, description: &'static str) -> CommandMeta {
    CommandMeta::new(name, description)
        .with_params(&[
            "browser",
            "registry",
            "workspace",
            "workspaces",
            "include-workspace-root",
        ])
        .with_usage(&["[<pkgname> [<pkgname> ...]]"])
        .workspaces()
}

/// Reads `repository` (string or `{url}`) from a manifest and maps it onto a
/// known hosted provider URL.
pub fn hosted_url(mani: &Value, kind: HostedKind) -> Option<String> {
    let repo = mani.get("repository")?;
    let rurl = match repo {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("url")?.as_str()?.to_string(),
        _ => return None,
    };
    let rurl = rurl.trim_start_matches("git+");
    let parsed = url::Url::parse(rurl).ok()?;
    let host = parsed.host_str()?;
    let path = parsed.path().trim_start_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let user = parts.next()?;
    let project = path.split('/').nth(1).unwrap_or("");
    if project.is_empty() {
        return None;
    }
    let base = match host {
        "github.com" => "https://github.com",
        "gitlab.com" => "https://gitlab.com",
        "bitbucket.org" => "https://bitbucket.org",
        _ => return None,
    };
    Some(match kind {
        HostedKind::Bugs => format!("{base}/{user}/{project}/issues"),
        HostedKind::Docs => format!("{base}/{user}/{project}#readme"),
        HostedKind::Browse => format!("{base}/{user}/{project}"),
        HostedKind::Https => format!("https://{host}/{path}"),
    })
}

#[derive(Debug, Clone, Copy)]
pub enum HostedKind {
    Bugs,
    Docs,
    Browse,
    Https,
}
