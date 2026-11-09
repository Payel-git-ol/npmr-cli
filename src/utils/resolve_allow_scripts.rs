//! Resolves the effective `allowScripts` policy from CLI, `.npmrc` and
//! `package.json`, ported from `resolve-allow-scripts.js`.

use std::path::Path;

use serde_json::Value;

use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::allow_scripts_writer::{Policy, PolicyKey, parse_key};
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicySource {
    Cli,
    PackageJson,
    Npmrc,
}

impl PolicySource {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicySource::Cli => "cli",
            PolicySource::PackageJson => "package.json",
            PolicySource::Npmrc => ".npmrc",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPolicy {
    pub policy: Option<Policy>,
    pub source: Option<PolicySource>,
}

fn parse_allow_list(value: &str) -> Vec<String> {
    value
        .split([' ', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn policy_from_values(values: &[Value]) -> Option<Policy> {
    let mut policy = Policy::new();
    for value in values {
        let names = match value {
            Value::String(s) => parse_allow_list(s),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        };
        for name in names {
            policy.insert(name, Value::Bool(true));
        }
    }
    (!policy.is_empty()).then_some(policy)
}

/// Drops keys with forbidden semver ranges or dist-tags, warning about each.
fn validate_policy(policy: Option<Policy>, source_label: &str) -> Option<Policy> {
    let policy = policy?;
    let mut cleaned = Policy::new();
    for (key, value) in policy {
        let Some(parsed) = parse_key(&key) else {
            warn(
                "install-scripts",
                &[&format!("{source_label}: ignoring unparseable entry \"{key}\"")],
            );
            continue;
        };
        match parsed {
            PolicyKey::Versions(_, versions) => {
                let allowed = versions.len() == 1
                    || key
                        .split("||")
                        .all(|part| part.split_once('@').is_some_and(|(_, v)| v.contains('.')));
                if !allowed {
                    warn(
                        "install-scripts",
                        &[&format!(
                            "{source_label}: ignoring \"{key}\" — semver ranges (^, ~, >=, <) are not allowed; \
                             use exact versions joined by \"||\" instead"
                        )],
                    );
                    continue;
                }
                cleaned.insert(key, value);
            }
            PolicyKey::Other(spec) if spec.starts_with("git") || spec.contains("://") => {
                warn(
                    "install-scripts",
                    &[&format!("{source_label}: ignoring unparseable entry \"{key}\"")],
                );
            }
            _ => {
                cleaned.insert(key, value);
            }
        }
    }
    (!cleaned.is_empty()).then_some(cleaned)
}

fn project_allow_scripts(prefix: &Path) -> Option<Policy> {
    let text = std::fs::read_to_string(prefix.join("package.json")).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    let map = manifest.get("allowScripts")?.as_object()?;
    (!map.is_empty()).then(|| map.clone())
}

/// Resolves the policy using RFC npm/rfcs#868 precedence.
pub fn resolve_allow_scripts(npm: &dyn Npm, skip_project_config: bool) -> Result<ResolvedPolicy> {
    let store = npm.store();
    let cli_values = store
        .get_at("allow-scripts", ConfigSource::Cli)
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    let rc_values = [
        ConfigSource::Project,
        ConfigSource::User,
        ConfigSource::Global,
        ConfigSource::Builtin,
    ]
    .iter()
    .filter_map(|source| store.get_at("allow-scripts", *source).cloned())
    .collect::<Vec<_>>();

    let cli_policy = policy_from_values(&cli_values);
    let npmrc_policy = policy_from_values(&rc_values);

    if cli_policy.is_some() && !npm.global() && !skip_project_config {
        return Err(NpmError::new(
            "--allow-scripts is not allowed in project-scoped installs. \
             Add the entries to the \"allowScripts\" field in package.json, \
             or to .npmrc, instead.",
        )
        .with_code("EALLOWSCRIPTS"));
    }

    let pkg_policy = if !npm.global() && !skip_project_config {
        project_allow_scripts(&npm.prefix())
    } else {
        None
    };

    let cli = validate_policy(cli_policy, "CLI flag");
    let pkg = validate_policy(pkg_policy, "package.json");
    let rc = validate_policy(npmrc_policy, ".npmrc");

    if let Some(policy) = cli {
        if rc.is_some() {
            warn(
                "install-scripts",
                &[&".npmrc allow-scripts setting is being ignored because --allow-scripts was passed on the command line"],
            );
        }
        return Ok(ResolvedPolicy {
            policy: Some(policy),
            source: Some(PolicySource::Cli),
        });
    }
    if let Some(policy) = pkg {
        if rc.is_some() {
            warn(
                "install-scripts",
                &[&".npmrc allow-scripts setting is being ignored because package.json declares its own allowScripts field"],
            );
        }
        return Ok(ResolvedPolicy {
            policy: Some(policy),
            source: Some(PolicySource::PackageJson),
        });
    }
    if let Some(policy) = rc {
        return Ok(ResolvedPolicy {
            policy: Some(policy),
            source: Some(PolicySource::Npmrc),
        });
    }
    Ok(ResolvedPolicy {
        policy: None,
        source: None,
    })
}
