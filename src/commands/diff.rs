//! Port of `lib/commands/diff.js`. Spec resolution (which two tarballs to
//! compare) is ported; producing the actual diff needs `libnpmdiff`, which is
//! still stubbed, so the command reports `ENOTSUP` after resolving.

use std::path::PathBuf;

use semver::VersionReq;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{info, verbose};

pub static META: CommandMeta = CommandMeta::new("diff", "The registry diff command")
    .with_params(&[
        "diff",
        "diff-name-only",
        "diff-unified",
        "diff-ignore-all-space",
        "diff-no-prefix",
        "diff-src-prefix",
        "diff-dst-prefix",
        "diff-text",
        "global",
        "tag",
        "workspace",
        "workspaces",
        "include-workspace-root",
    ])
    .with_usage(&["[...<paths>]"])
    .workspaces();

pub struct Diff;

impl Diff {
    pub fn new() -> Self {
        Self
    }

    fn diff_at(&self, npm: &dyn Npm, top: &std::path::Path, args: &[String]) -> Result<()> {
        let specs: Vec<String> = npm
            .flat_options()
            .get("diff")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if specs.len() > 2 {
            return Err(META.usage_error("Can't use more than two --diff arguments."));
        }

        let (src, dst) = self.retrieve_specs(npm, top, &specs)?;
        info("diff", &[&"src", &src, &"dst", &"", &dst]);
        let _files = args;
        Err(NpmError::new(format!(
            "Cannot diff {src} against {dst}: comparing tarballs is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }

    fn package_name(&self, top: &std::path::Path) -> Result<String> {
        let name = std::fs::read_to_string(top.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("name").and_then(serde_json::Value::as_str).map(str::to_string));
        match name {
            Some(name) if !name.is_empty() => Ok(name),
            _ => {
                verbose("diff", &[&"could not read project dir package.json"]);
                Err(META.usage_error(
                    "Needs multiple arguments to compare or run from a project dir.",
                ))
            }
        }
    }

    fn retrieve_specs(
        &self,
        npm: &dyn Npm,
        top: &std::path::Path,
        specs: &[String],
    ) -> Result<(String, String)> {
        let prefix = top.display().to_string();
        let a = specs.first().cloned();
        let b = specs.get(1).cloned();
        if let (Some(a), Some(b)) = (&a, &b) {
            let (ga, gb) = convert_versions_to_specs(npm, top, a, b)?;
            return Ok((ga, gb));
        }
        let Some(a) = a else {
            let pkg_name = self.package_name(top)?;
            let tag = npm.store().get_str("tag");
            return Ok((format!("{pkg_name}@{tag}"), format!("file:{prefix}")));
        };

        let pkg_name = std::fs::read_to_string(top.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("name").and_then(serde_json::Value::as_str).map(str::to_string))
            .filter(|n| !n.is_empty());
        if pkg_name.is_none() {
            verbose("diff", &[&"could not read project dir package.json"]);
        }
        let missing = || META.usage_error("Needs multiple arguments to compare or run from a project dir.");

        if is_version_req(&a) {
            let Some(pkg_name) = pkg_name else {
                return Err(missing());
            };
            return Ok((format!("{pkg_name}@{a}"), format!("file:{prefix}")));
        }

        if a.starts_with("file:") || a.starts_with('.') || a.starts_with('/') {
            let path = a.trim_start_matches("file:");
            return Ok((format!("file:{path}"), format!("file:{prefix}")));
        }
        if a.contains("://") || a.starts_with("git") || a.starts_with("github:") {
            return Err(META.usage_error(format!("Spec type {} not supported.", "git")));
        }

        let Some(pkg_name) = pkg_name else {
            return Err(missing());
        };
        let _ = pkg_name;
        let (name, raw_spec) = split_spec(&a);
        let raw_spec = if raw_spec.is_empty() { "*".to_string() } else { raw_spec };
        Err(NpmError::new(format!(
            "Cannot diff {name}@{raw_spec}: reading the install tree and registry is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}

impl Default for Diff {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Diff {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let top = if npm.global() {
            let global_dir = npm.global_dir();
            global_dir.parent().unwrap_or(&global_dir).to_path_buf()
        } else {
            npm.prefix()
        };
        self.diff_at(&*npm, &top, args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        for path in workspaces.values() {
            self.diff_at(&*npm, &PathBuf::from(path), args)?;
        }
        Ok(())
    }
}

fn convert_versions_to_specs(
    npm: &dyn Npm,
    top: &std::path::Path,
    a: &str,
    b: &str,
) -> Result<(String, String)> {
    let _ = npm;
    let semver_a = is_version_req(a);
    let semver_b = is_version_req(b);
    let pkg_name = std::fs::read_to_string(top.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("name").and_then(serde_json::Value::as_str).map(str::to_string))
        .filter(|n| !n.is_empty());

    if semver_a && semver_b {
        let Some(pkg_name) = pkg_name else {
            return Err(META.usage_error(
                "Needs to be run from a project dir in order to diff two versions.",
            ));
        };
        return Ok((format!("{pkg_name}@{a}"), format!("{pkg_name}@{b}")));
    }
    if !semver_a && semver_b {
        return Ok((a.to_string(), format!("{}@{b}", split_spec(a).0)));
    }
    if semver_a && !semver_b {
        return Ok((format!("{}@{a}", split_spec(b).0), b.to_string()));
    }
    Ok((a.to_string(), b.to_string()))
}

fn split_spec(spec: &str) -> (String, String) {
    match spec.rfind('@') {
        Some(0) | None => (spec.to_string(), String::new()),
        Some(idx) => (spec[..idx].to_string(), spec[idx + 1..].to_string()),
    }
}

/// Whether the argument parses as a semver range (`1.2.3`, `^1`, `*`, ...).
fn is_version_req(text: &str) -> bool {
    VersionReq::parse(text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{is_version_req, split_spec};

    #[test]
    fn ranges_are_recognised() {
        assert!(is_version_req("*"));
        assert!(is_version_req("1.2.3"));
        assert!(is_version_req("^1.0.0"));
        assert!(!is_version_req("lodash"));
        assert!(!is_version_req("@scope/pkg@latest"));
    }

    #[test]
    fn splits_specs() {
        assert_eq!(split_spec("lodash"), ("lodash".into(), String::new()));
        assert_eq!(split_spec("lodash@4"), ("lodash".into(), "4".into()));
        assert_eq!(split_spec("@a/b"), ("@a/b".into(), String::new()));
    }
}
