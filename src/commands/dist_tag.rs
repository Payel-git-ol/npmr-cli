//! Port of `lib/commands/dist-tag.js`. Registry traffic is stubbed, so
//! `add`/`rm`/`ls` validate their arguments and then report `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{info, verbose, warn};

pub static META: CommandMeta =
    CommandMeta::new("dist-tag", "Modify package distribution tags")
        .with_params(&["workspace", "workspaces", "include-workspace-root"])
        .with_usage(&[
            "add <package-spec (with version)> [<tag>]",
            "rm <package-spec> <tag>",
            "ls [<package-spec>]",
        ])
        .workspaces();

pub struct DistTag;

impl DistTag {
    pub fn new() -> Self {
        Self
    }

    fn add(&self, spec: &str, tag: Option<&str>, default_tag: &str) -> Result<()> {
        let (name, version) = split_spec(spec);
        let tag = tag.unwrap_or(default_tag).to_string();
        let target = name.as_ref().map(|n| format!("{n}@{version}")).unwrap_or_default();
        verbose("dist-tag add", &[&tag, &"to", &target]);
        let Some(name) = name else {
            return Err(META.usage_error(
                "must provide a spec with a name and version, and a tag to add",
            ));
        };
        if version.is_empty() || tag.is_empty() {
            return Err(META.usage_error(
                "must provide a spec with a name and version, and a tag to add",
            ));
        }
        let t = tag.trim();
        if is_semver_range(t) {
            return Err(NpmError::new(format!(
                "Tag name must not be a valid SemVer range: {t}"
            )));
        }
        Err(registry_stub(&name))
    }

    fn remove(&self, spec: &str, tag: Option<&str>) -> Result<()> {
        let (name, _) = split_spec(spec);
        let from = name.clone().unwrap_or_default();
        verbose("dist-tag del", &[&tag.unwrap_or(""), &"from", &from]);
        let Some(name) = name else {
            return Err(META.usage_error(""));
        };
        let Some(tag) = tag.filter(|t| !t.is_empty()) else {
            return Err(META.usage_error(""));
        };
        info("dist-tag del", &[&tag, &"is not a dist-tag on", &name]);
        Err(NpmError::new(format!("{tag} is not a dist-tag on {name}"))
            .with_detail(registry_stub(&name).body))
    }

    fn list(&self, npm: &dyn Npm, spec: Option<&str>) -> Result<()> {
        let name = match spec {
            Some(spec) if !spec.is_empty() => split_spec(spec).0.unwrap_or_default(),
            _ => {
                if npm.global() {
                    return Err(META.usage_error(""));
                }
                let package = npm.prefix().join("package.json");
                let manifest = std::fs::read_to_string(package)
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .unwrap_or_default();
                manifest
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            }
        };
        if name.is_empty() {
            return Err(META.usage_error(""));
        }
        crate::utils::log::error("dist-tag ls", &[&"Couldn't get dist-tag data for", &name]);
        Err(registry_stub(&name))
    }

    fn list_workspaces(&self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        for name in workspaces.keys() {
            crate::output::standard(format!("{name}:"));
            if let Err(_err) = self.list(&*npm, Some(name)) {
                crate::output::set_exit_code(1);
            }
        }
        Ok(())
    }

    fn exec_switch(&self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        let pkg = args.get(1).map(String::as_str);
        let tag = args.get(2).map(String::as_str);
        match cmd {
            "add" | "a" | "set" | "s" => self.add(pkg.unwrap_or(""), tag, &npm.store().get_str("tag")),
            "rm" | "r" | "del" | "d" | "remove" => self.remove(pkg.unwrap_or(""), tag),
            "ls" | "l" | "sl" | "list" => self.list(&*npm, pkg),
            _ if pkg.is_none() => self.list(&*npm, (!cmd.is_empty()).then_some(cmd)),
            _ => Err(META.usage_error("")),
        }
    }
}

impl Default for DistTag {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for DistTag {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec_switch(npm, args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        let pkg = args.get(1).map(String::as_str);
        let is_list = matches!(cmd, "ls" | "l" | "sl" | "list");
        let local_only = |value: Option<&str>| {
            value.is_none_or(|v| v == "." || v.starts_with(".@"))
        };
        if is_list && local_only(pkg) || pkg.is_none() && local_only(Some(cmd)) {
            return self.list_workspaces(npm);
        }
        warn("dist-tag", &[&"Ignoring workspaces for specified package"]);
        self.exec_switch(npm, args)
    }
}

fn registry_stub(name: &str) -> NpmError {
    NpmError::new(format!(
        "Cannot read dist-tags for {name}: registry access is not implemented yet"
    ))
    .with_code("ENOTSUP")
}

/// Splits `name@version`; a scoped name keeps its leading `@`.
fn split_spec(spec: &str) -> (Option<String>, String) {
    match spec.rfind('@') {
        Some(0) | None => (Some(spec.to_string()).filter(|s| !s.is_empty()), String::new()),
        Some(idx) => (Some(spec[..idx].to_string()), spec[idx + 1..].to_string()),
    }
}

fn is_semver_range(text: &str) -> bool {
    text.split("||").all(|part| {
        let part = part.trim();
        if part == "*" || part == "x" || part.is_empty() {
            return true;
        }
        part.split_whitespace().all(|token| {
            let stripped = token.trim_start_matches(['>', '<', '=', '~', '^']);
            stripped
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
                || matches!(stripped, "x" | "X" | "*")
        })
    }) && text.chars().any(|c| c.is_ascii_digit() || c == '*')
}

#[cfg(test)]
mod tests {
    use super::{is_semver_range, split_spec};

    #[test]
    fn splits_scoped_and_plain_names() {
        assert_eq!(split_spec("foo@1.0.0"), (Some("foo".into()), "1.0.0".into()));
        assert_eq!(split_spec("@a/b@next"), (Some("@a/b".into()), "next".into()));
        assert_eq!(split_spec("@a/b"), (Some("@a/b".into()), String::new()));
    }

    #[test]
    fn detects_ranges() {
        assert!(is_semver_range("1.2.3"));
        assert!(is_semver_range("^1.0.0"));
        assert!(!is_semver_range("next"));
    }
}
