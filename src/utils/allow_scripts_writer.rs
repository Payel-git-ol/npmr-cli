//! Shared helpers for the install-script allowlist ported from
//! `allow-scripts-*.js`. Arborist is not available yet, so "nodes" are
//! reduced to the manifest information the policy logic needs.

use std::path::Path;

use semver::Version;
use serde_json::Value;

/// The parts of an installed dependency the allowlist logic cares about.
#[derive(Debug, Clone)]
pub struct ScriptNode {
    pub name: String,
    pub version: String,
    /// `resolved` from the lockfile / manifest when known.
    pub resolved: Option<String>,
    pub path: String,
    /// `file:`, `directory:` or hosted git specs keep the spec string.
    pub spec: Option<String>,
    pub has_scripts: bool,
}

/// Lifecycle scripts that run on install, in the order arborist reports them.
pub const INSTALL_EVENTS: &[&str] = &["preinstall", "install", "postinstall"];

/// Collects the install scripts a manifest declares.
pub fn install_scripts(manifest: &Value) -> Vec<(String, String)> {
    let Some(scripts) = manifest.get("scripts").and_then(Value::as_object) else {
        return Vec::new();
    };
    INSTALL_EVENTS
        .iter()
        .filter_map(|event| {
            scripts
                .get(*event)
                .and_then(Value::as_str)
                .map(|cmd| ((*event).to_string(), cmd.to_string()))
        })
        .collect()
}

/// Reads every installed package under `root`'s `node_modules` (one level,
/// plus `@scope` directories) into [`ScriptNode`]s.
pub fn scan_nodes(root: &Path) -> Vec<ScriptNode> {
    let modules = root.join("node_modules");
    let Ok(entries) = std::fs::read_dir(&modules) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();

    let mut nodes = Vec::new();
    for name in names {
        let dir = modules.join(&name);
        if name.starts_with('@') {
            let Ok(scoped) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut subs: Vec<String> = scoped
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            subs.sort();
            for sub in subs {
                if let Some(node) = read_node(&dir.join(&sub)) {
                    nodes.push(node);
                }
            }
        } else if let Some(node) = read_node(&dir) {
            nodes.push(node);
        }
    }
    nodes
}

fn read_node(dir: &Path) -> Option<ScriptNode> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    let name = manifest.get("name").and_then(Value::as_str)?.to_string();
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(ScriptNode {
        name,
        version,
        resolved: manifest
            .get("_resolved")
            .and_then(Value::as_str)
            .map(str::to_string),
        path: dir.to_string_lossy().into_owned(),
        spec: None,
        has_scripts: !install_scripts(&manifest).is_empty(),
    })
}

/// A parsed `allowScripts` key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyKey {
    /// Name only: `pkg`, `pkg@*` or `pkg@latest`.
    Name(String),
    /// Exact versions joined by `||`: `pkg@1.2.3||pkg@1.2.4`.
    Versions(String, Vec<Version>),
    /// Anything else we cannot reliably match against (git, file, remote).
    Other(String),
}

/// Parses a policy key the way `npm-package-arg` would classify it.
pub fn parse_key(key: &str) -> Option<PolicyKey> {
    if key.trim().is_empty() {
        return None;
    }
    if key.starts_with("file:") || key.starts_with("git") || key.contains("://") {
        return Some(PolicyKey::Other(key.to_string()));
    }
    // `a@1.2.3||a@1.2.4` is one key naming several pins.
    let alternatives: Vec<&str> = key.split("||").map(str::trim).collect();
    let (name, spec) = match alternatives[0].rfind('@') {
        Some(0) | None => (alternatives[0], None),
        Some(idx) => (&alternatives[0][..idx], Some(&alternatives[0][idx + 1..])),
    };
    if name.is_empty() {
        return None;
    }
    let Some(spec) = spec else {
        return Some(PolicyKey::Name(name.to_string()));
    };
    if spec.is_empty() || spec == "*" || !is_semver_like(spec) {
        return Some(PolicyKey::Name(name.to_string()));
    }
    let mut versions = Vec::new();
    for part in &alternatives {
        let version = match part.rfind('@') {
            Some(idx) => &part[idx + 1..],
            None => part,
        };
        let Ok(version) = Version::parse(version.trim_start_matches('=')) else {
            return Some(PolicyKey::Name(name.to_string()));
        };
        versions.push(version);
    }
    Some(PolicyKey::Versions(name.to_string(), versions))
}

fn is_semver_like(spec: &str) -> bool {
    let first = spec.split("||").next().unwrap_or("");
    let version = match first.trim().rfind('@') {
        Some(idx) => &first[idx + 1..],
        None => first.trim(),
    };
    version
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '=')
}

/// Whether a key matches the node's trusted identity.
pub fn key_targets_node(key: &PolicyKey, node: &ScriptNode) -> bool {
    match key {
        PolicyKey::Name(name) | PolicyKey::Versions(name, _) => *name == node.name,
        PolicyKey::Other(spec) => {
            node.resolved.as_deref() == Some(spec.as_str())
                || node.spec.as_deref() == Some(spec.as_str())
        }
    }
}

/// Whether the key is a name-only wildcard.
pub fn is_name_only(key: &PolicyKey) -> bool {
    matches!(key, PolicyKey::Name(_))
}

/// The spec written for a single-version pin, if the node has a trusted
/// version. Manifests without a lockfile `resolved` URL cannot be pinned.
pub fn versioned_key(node: &ScriptNode) -> Option<String> {
    node.resolved.as_ref()?;
    if node.version.is_empty() {
        return None;
    }
    Some(format!("{}@{}", node.name, node.version))
}

/// The spec used for a name-only entry.
pub fn name_key(node: &ScriptNode) -> Option<String> {
    if node.name.is_empty() {
        return None;
    }
    Some(node.name.clone())
}

/// A single-version pin such as `pkg@1.2.3`.
pub fn is_single_version_pin(key: &PolicyKey) -> bool {
    matches!(key, PolicyKey::Versions(_, versions) if versions.len() == 1)
}

pub type Policy = serde_json::Map<String, Value>;

pub struct PolicyChange {
    pub key: String,
    pub change: &'static str,
}

pub struct PolicyResult {
    pub allow_scripts: Policy,
    pub changes: Vec<PolicyChange>,
    pub warning: Option<String>,
}

fn deny_warning(key: &str, subject: &str, name: Option<&str>) -> String {
    match parse_key(key) {
        Some(PolicyKey::Name(_)) => {
            format!("{key} is denied; remove the entry from allowScripts to approve {subject}.")
        }
        _ => {
            let target = name.unwrap_or("this package");
            format!(
                "{key} is a versioned deny; run `npm install-scripts deny {target}` \
                 to widen the deny to all versions of {target}, or remove the entry \
                 to approve {subject}."
            )
        }
    }
}

/// Approves every installed version of a single package. Implements the RFC's
/// pin-mismatch table; the input map is never mutated.
pub fn apply_approval_for_package(
    existing: &Policy,
    nodes: &[&ScriptNode],
    pin: bool,
) -> PolicyResult {
    let mut allow_scripts = existing.clone();
    let mut changes = Vec::new();
    let Some(sample) = nodes.first() else {
        return PolicyResult {
            allow_scripts,
            changes,
            warning: None,
        };
    };
    let name = name_key(sample);

    let denied = nodes.iter().find_map(|node| {
        allow_scripts.iter().find_map(|(key, value)| {
            (value.as_bool() == Some(false))
                .then(|| parse_key(key))
                .flatten()
                .filter(|parsed| key_targets_node(parsed, node))
                .map(|_| key.clone())
        })
    });
    if let Some(key) = denied {
        let subject = name.as_deref().unwrap_or("this package");
        return PolicyResult {
            allow_scripts,
            changes,
            warning: Some(deny_warning(&key, subject, name.as_deref())),
        };
    }

    let remove_if = |allow_scripts: &mut Policy,
                     changes: &mut Vec<PolicyChange>,
                     change: &'static str,
                     predicate: &dyn Fn(&str) -> bool| {
        for key in keys(allow_scripts) {
            if predicate(&key) && allow_scripts.get(&key).and_then(Value::as_bool) == Some(true) {
                allow_scripts.remove(&key);
                changes.push(PolicyChange { key, change });
            }
        }
    };

    if !pin {
        let sample_name = name.clone();
        remove_if(
            &mut allow_scripts,
            &mut changes,
            "removed-pinned-allow",
            &|key| {
                parse_key(key).is_some_and(|parsed| {
                    key_targets_node(&parsed, sample)
                        && Some(key.to_string()) != sample_name
                        && is_single_version_pin(&parsed)
                })
            },
        );
        if let Some(name) = name {
            if allow_scripts.get(&name).and_then(Value::as_bool) != Some(true) {
                allow_scripts.insert(name.clone(), Value::Bool(true));
                changes.push(PolicyChange {
                    key: name,
                    change: "added",
                });
            }
        }
        return PolicyResult {
            allow_scripts,
            changes,
            warning: None,
        };
    }

    let versioned = nodes.iter().map(|n| versioned_key(n)).collect::<Vec<_>>();
    let installed: Vec<String> = versioned.iter().flatten().cloned().collect();

    if let Some(name) = &name {
        if versioned.iter().any(Option::is_none) {
            remove_if(
                &mut allow_scripts,
                &mut changes,
                "removed-pinned-allow",
                &|key| {
                    parse_key(key).is_some_and(|parsed| {
                        key_targets_node(&parsed, sample)
                            && key != name
                            && is_single_version_pin(&parsed)
                    })
                },
            );
            if allow_scripts.get(name).and_then(Value::as_bool) != Some(true) {
                allow_scripts.insert(name.clone(), Value::Bool(true));
                changes.push(PolicyChange {
                    key: name.clone(),
                    change: "added",
                });
            }
            let warning = (!changes.is_empty()).then(|| {
                format!(
                    "{name}: approved by name (all versions) because its \
                     package-lock.json entry has no \"resolved\" URL, so npm can't pin a \
                     specific version. Run `npm install` to refresh the lockfile and \
                     enable pinning."
                )
            });
            return PolicyResult {
                allow_scripts,
                changes,
                warning,
            };
        }
    }

    remove_if(&mut allow_scripts, &mut changes, "removed-stale", &|key| {
        parse_key(key).is_some_and(|parsed| {
            key_targets_node(&parsed, sample)
                && is_single_version_pin(&parsed)
                && !installed.iter().any(|i| i == key)
        })
    });

    for key in &installed {
        if allow_scripts.get(key).and_then(Value::as_bool) != Some(true) {
            allow_scripts.insert(key.clone(), Value::Bool(true));
            changes.push(PolicyChange {
                key: key.clone(),
                change: "added",
            });
        }
    }

    if !installed.is_empty() {
        if let Some(name) = name {
            if !installed.iter().any(|i| i == &name)
                && allow_scripts.get(&name).and_then(Value::as_bool) == Some(true)
            {
                allow_scripts.remove(&name);
                changes.push(PolicyChange {
                    key: name,
                    change: "replaced-by-pin",
                });
            }
        }
    }

    PolicyResult {
        allow_scripts,
        changes,
        warning: None,
    }
}

/// Denies a package by name, dropping any pins for it.
pub fn apply_deny_for_package(existing: &Policy, nodes: &[&ScriptNode]) -> PolicyResult {
    let mut allow_scripts = existing.clone();
    let mut changes = Vec::new();
    let Some(sample) = nodes.first() else {
        return PolicyResult {
            allow_scripts,
            changes,
            warning: None,
        };
    };
    let Some(name) = name_key(sample) else {
        return PolicyResult {
            allow_scripts,
            changes,
            warning: None,
        };
    };

    for key in keys(&allow_scripts) {
        if key == name {
            continue;
        }
        if parse_key(&key).is_some_and(|parsed| key_targets_node(&parsed, sample)) {
            allow_scripts.remove(&key);
            changes.push(PolicyChange {
                key,
                change: "removed-pinned-allow",
            });
        }
    }

    if allow_scripts.get(&name).and_then(Value::as_bool) != Some(false) {
        allow_scripts.insert(name.clone(), Value::Bool(false));
        changes.push(PolicyChange {
            key: name,
            change: "added",
        });
    }
    PolicyResult {
        allow_scripts,
        changes,
        warning: None,
    }
}

fn keys(policy: &Policy) -> Vec<String> {
    policy.keys().cloned().collect()
}

pub struct RemovedEntry {
    pub key: String,
    pub reason: &'static str,
}

/// Splits `allowScripts` into entries to keep and unused ones to drop.
pub fn classify_unused_entries(
    existing: &Policy,
    nodes: &[&ScriptNode],
) -> (Policy, Vec<RemovedEntry>) {
    let mut remaining = Policy::new();
    let mut removed = Vec::new();
    for (key, value) in existing {
        let Some(parsed) = parse_key(key) else {
            remaining.insert(key.clone(), value.clone());
            continue;
        };
        let matching: Vec<&&ScriptNode> = nodes
            .iter()
            .filter(|node| key_targets_node(&parsed, node))
            .collect();
        if matching.is_empty() {
            removed.push(RemovedEntry {
                key: key.clone(),
                reason: "not-installed",
            });
            continue;
        }
        if !matching.iter().any(|node| node.has_scripts) {
            removed.push(RemovedEntry {
                key: key.clone(),
                reason: "no-scripts",
            });
            continue;
        }
        remaining.insert(key.clone(), value.clone());
    }
    (remaining, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_pin_keys() {
        assert_eq!(parse_key("canvas"), Some(PolicyKey::Name("canvas".into())));
        assert_eq!(
            parse_key("canvas@*"),
            Some(PolicyKey::Name("canvas".into()))
        );
        assert_eq!(parse_key("@a/b"), Some(PolicyKey::Name("@a/b".into())));
        assert!(matches!(
            parse_key("canvas@1.2.3"),
            Some(PolicyKey::Versions(_, v)) if v.len() == 1
        ));
        assert!(matches!(
            parse_key("canvas@1.2.3||canvas@1.2.4"),
            Some(PolicyKey::Versions(_, v)) if v.len() == 2
        ));
    }

    #[test]
    fn pins_when_resolved_is_known() {
        let existing = Policy::new();
        let node = ScriptNode {
            name: "canvas".into(),
            version: "1.2.3".into(),
            resolved: Some("https://registry.npmjs.org/canvas/-/canvas-1.2.3.tgz".into()),
            path: "/x".into(),
            spec: None,
            has_scripts: true,
        };
        let result = apply_approval_for_package(&existing, &[&node], true);
        assert_eq!(
            result.allow_scripts.get("canvas@1.2.3"),
            Some(&Value::Bool(true))
        );
    }
}
