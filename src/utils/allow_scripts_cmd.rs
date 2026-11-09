//! Port of `lib/utils/allow-scripts-cmd.js`: shared implementation for
//! `npm approve-scripts`, `npm deny-scripts` and the `npm install-scripts`
//! namespace.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::commands::CommandMeta;
use crate::npm::Npm;
use crate::utils::allow_scripts_writer::{
    ScriptNode, install_scripts, scan_nodes,
};
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;
use crate::utils::resolve_allow_scripts::resolve_allow_scripts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Approve,
    Deny,
    List,
    Prune,
}

impl Mode {
    pub fn verb(self) -> &'static str {
        match self {
            Mode::Approve => "approve",
            Mode::Deny => "deny",
            Mode::List => "list",
            Mode::Prune => "prune",
        }
    }
}

/// Which flags the calling command declares.
#[derive(Debug, Clone, Copy)]
pub struct ModeOptions {
    pub pending_flag: bool,
}

fn parse_positional(arg: &str) -> (String, Option<String>) {
    let Some(parsed) = crate::utils::allow_scripts_writer::parse_key(arg) else {
        return (arg.to_string(), None);
    };
    match parsed {
        crate::utils::allow_scripts_writer::PolicyKey::Name(name) => (name, None),
        crate::utils::allow_scripts_writer::PolicyKey::Versions(name, versions) => {
            let range = versions.first().map(|v| v.to_string());
            (name, range)
        }
        crate::utils::allow_scripts_writer::PolicyKey::Other(_) => (arg.to_string(), None),
    }
}

fn version_satisfies(version: &str, range: &str) -> bool {
    let Ok(req) = semver::VersionReq::parse(range) else {
        return false;
    };
    semver::Version::parse(version)
        .map(|v| req.matches(&v))
        .unwrap_or(false)
}

fn find_nodes_for_args<'a>(args: &[String], nodes: &'a [ScriptNode]) -> (Vec<&'a ScriptNode>, Vec<String>) {
    let mut matched: Vec<&ScriptNode> = Vec::new();
    let mut unmatched = Vec::new();
    for arg in args {
        let (want, range) = parse_positional(arg);
        let found: Vec<&ScriptNode> = nodes
            .iter()
            .filter(|node| {
                node.name == want
                    && range
                        .as_deref()
                        .is_none_or(|r| version_satisfies(&node.version, r))
            })
            .collect();
        if found.is_empty() {
            unmatched.push(arg.clone());
        } else {
            for node in found {
                if !matched.iter().any(|n| n.path == node.path) {
                    matched.push(node);
                }
            }
        }
    }
    (matched, unmatched)
}

fn group_by_package<'a>(nodes: &[&'a ScriptNode]) -> Vec<(String, Vec<&'a ScriptNode>)> {
    let mut groups: Vec<(String, Vec<&ScriptNode>)> = Vec::new();
    for node in nodes {
        match groups.iter_mut().find(|(name, _)| *name == node.name) {
            Some((_, list)) => list.push(node),
            None => groups.push((node.name.clone(), vec![*node])),
        }
    }
    groups
}

fn pending_summary(entries: &[(&ScriptNode, Vec<(String, String)>)]) -> Value {
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for (node, _) in entries {
        let key = if node.version.is_empty() {
            node.name.clone()
        } else {
            format!("{}@{}", node.name, node.version)
        };
        match groups.iter_mut().find(|(name, _)| *name == node.name) {
            Some((_, changes)) => changes.push(json!({ "key": key, "change": "pending" })),
            None => groups.push((
                node.name.clone(),
                vec![json!({ "key": key, "change": "pending" })],
            )),
        }
    }
    Value::Array(
        groups
            .into_iter()
            .map(|(name, changes)| json!({ "name": name, "changes": changes }))
            .collect(),
    )
}

fn load_allow_scripts(prefix: &Path) -> Map<String, Value> {
    std::fs::read_to_string(prefix.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|v| v.get("allowScripts").and_then(Value::as_object).cloned())
        .unwrap_or_default()
}

fn save_allow_scripts(prefix: &Path, policy: &Map<String, Value>) -> Result<()> {
    let path = prefix.join("package.json");
    let text = std::fs::read_to_string(&path)?;
    let mut manifest: Value = serde_json::from_str(&text).map_err(|err| {
        NpmError::new(format!("Failed to parse json\n{err}")).with_code("EJSONPARSE")
    })?;
    let obj = manifest.as_object_mut().ok_or_else(|| {
        NpmError::new("package.json must contain an object").with_code("EJSONPARSE")
    })?;
    if policy.is_empty() {
        obj.remove("allowScripts");
    } else {
        obj.insert("allowScripts".into(), Value::Object(policy.clone()));
    }
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap_or_default()))?;
    Ok(())}

/// Runs one of the allowlist modes. `meta` is used for usage errors.
pub fn run_mode(
    meta: &CommandMeta,
    npm: &mut (dyn Npm + '_),
    mode: Mode,
    args: &[String],
    options: ModeOptions,
) -> Result<()> {
    if npm.global() {
        return Err(NpmError::new(format!(
            "`npm {}` does not work for global installs",
            meta.name
        ))
        .with_code("EGLOBAL"));
    }

    if mode == Mode::Prune {
        return run_prune(meta, npm, args);
    }

    let pending = options.pending_flag && npm.store().get_bool("allow-scripts-pending");
    let all = npm.store().get_bool("all");
    let list = mode == Mode::List || pending;

    if list && (!args.is_empty() || all) {
        let what = if mode == Mode::List {
            "`npm install-scripts ls`"
        } else {
            "`--allow-scripts-pending`"
        };
        return Err(meta.usage_error(format!(
            "{what} cannot be combined with positional arguments or `--all`."
        )));
    }
    if !list && !all && args.is_empty() {
        return Err(meta.usage_error(""));
    }
    if mode == Mode::Deny && pending {
        return Err(meta.usage_error(
            "`npm deny-scripts --allow-scripts-pending` is not supported; \
             run `npm install-scripts ls` to list unreviewed packages.",
        ));
    }

    let prefix = npm.prefix();
    let resolved = resolve_allow_scripts(npm, false)?;
    let policy = resolved.policy.clone();
    let prefix = prefix.to_path_buf();
    let nodes = scan_nodes(&prefix);
    let scripts = |node: &ScriptNode| {
        std::fs::read_to_string(Path::new(&node.path).join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|m| install_scripts(&m))
            .unwrap_or_default()
    };
    let unreviewed = crate::utils::check_allow_scripts::check_allow_scripts(
        &nodes,
        scripts,
        policy.as_ref(),
        npm.store().get_bool("ignore-scripts"),
        true,
    );
    let entries: Vec<(&ScriptNode, Vec<(String, String)>)> =
        unreviewed.iter().map(|e| (e.node, e.scripts.clone())).collect();

    if list {
        print_pending(npm, &entries);
        return Ok(());
    }
    if all {
        if entries.is_empty() {
            if npm.json() {
                crate::output::set_meta(true);
                crate::output::standard(r#"{"allowScripts":[]}"#);
                crate::output::flush();
            } else {
                crate::output::standard("No packages with unreviewed install scripts.");
            }
            return Ok(());
        }
        let all_nodes: Vec<&ScriptNode> = entries.iter().map(|(n, _)| *n).collect();
        let groups = group_by_package(&all_nodes);
        return write_policy_changes(npm, mode, &prefix, &groups);
    }

    let (matched, unmatched) = find_nodes_for_args(args, &nodes);
    if !unmatched.is_empty() {
        return Err(NpmError::new(format!(
            "No installed packages match: {}",
            unmatched.join(", ")
        ))
        .with_code("ENOMATCH"));
    }
    if matched.is_empty() {
        return Err(NpmError::new(format!(
            "No installed packages match: {}",
            args.join(", ")
        ))
        .with_code("ENOMATCH"));
    }
    let groups = group_by_package(&matched);
    write_policy_changes(npm, mode, &prefix, &groups)
}

fn print_pending(npm: &dyn Npm, entries: &[(&ScriptNode, Vec<(String, String)>)]) {
    if npm.json() {
        crate::output::set_meta(true);
        crate::output::standard(
            json!({ "allowScripts": pending_summary(entries) }).to_string(),
        );
        crate::output::flush();
        return;
    }
    if entries.is_empty() {
        crate::output::standard("No packages with unreviewed install scripts.");
        return;
    }
    let count = entries.len();
    crate::output::standard(format!(
        "{count} {} {} install scripts blocked because they are not covered by allowScripts:",
        if count == 1 { "package" } else { "packages" },
        if count == 1 { "has" } else { "have" },
    ));
    for (node, scripts) in entries {
        let version = if node.version.is_empty() {
            String::new()
        } else {
            format!("@{}", node.version)
        };
        let events = scripts
            .iter()
            .map(|(event, cmd)| format!("{event}: {cmd}"))
            .collect::<Vec<_>>()
            .join("; ");
        crate::output::standard(format!("  {}{version} ({events})", node.name));
    }
    crate::output::standard("");
    crate::output::standard(
        "Run `npm install-scripts approve <pkg>` to allow, \
         or `npm install-scripts deny <pkg>` to deny.",
    );
}

fn write_policy_changes(
    npm: &(dyn Npm + '_),
    mode: Mode,
    prefix: &Path,
    groups: &[(String, Vec<&ScriptNode>)],
) -> Result<()> {
    let pin = npm.store().get_bool("allow-scripts-pin");
    let mut updated = load_allow_scripts(prefix);
    let original = updated.clone();
    let mut summary = Vec::new();

    for (name, nodes) in groups {
        let result = if mode == Mode::Approve {
            crate::utils::allow_scripts_writer::apply_approval_for_package(&updated, nodes, pin)
        } else {
            crate::utils::allow_scripts_writer::apply_deny_for_package(&updated, nodes)
        };
        if let Some(warning) = result.warning {
            warn("install-scripts", &[&warning]);
        }
        updated = result.allow_scripts;
        summary.push((
            name.clone(),
            result
                .changes
                .into_iter()
                .map(|c| json!({ "key": c.key, "change": c.change }))
                .collect::<Vec<_>>(),
        ));
    }

    if updated != original {
        save_allow_scripts(prefix, &updated)?;
    }
    print_summary(npm, mode, &summary);
    Ok(())
}

fn print_summary(npm: &dyn Npm, mode: Mode, summary: &[(String, Vec<Value>)]) {
    if npm.json() {
        let payload = summary
            .iter()
            .map(|(name, changes)| json!({ "name": name, "changes": changes }))
            .collect::<Vec<_>>();
        crate::output::set_meta(true);
        crate::output::standard(json!({ "allowScripts": payload }).to_string());
        crate::output::flush();
        return;
    }
    let verb = if mode == Mode::Approve {
        "Approved"
    } else {
        "Denied"
    };
    let mut touched = 0;
    for (name, changes) in summary {
        if changes.is_empty() {
            continue;
        }
        touched += 1;
        crate::output::standard(format!("{verb} {name}:"));
        for change in changes {
            let key = change.get("key").and_then(Value::as_str).unwrap_or_default();
            let what = change.get("change").and_then(Value::as_str).unwrap_or_default();
            crate::output::standard(format!("  {what} {key}"));
        }
    }
    if touched == 0 {
        crate::output::standard(format!("Nothing to {}; allowScripts unchanged.", mode.verb()));
    }
}

fn run_prune(meta: &CommandMeta, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
    let all = npm.store().get_bool("all");
    if !args.is_empty() || all {
        return Err(meta.usage_error(
            "`npm install-scripts prune` cannot be combined with positional arguments or `--all`.",
        ));
    }
    let dry_run = npm.store().get_bool("dry-run");
    let prefix = npm.prefix();
    let existing = load_allow_scripts(&prefix);
    let mut removed = Vec::new();

    if !existing.is_empty() {
        let nodes = scan_nodes(&prefix);
        let refs: Vec<&ScriptNode> = nodes.iter().collect();
        let (remaining, unused) =
            crate::utils::allow_scripts_prune::classify(&existing, &refs);
        removed = unused;
        if !removed.is_empty() && !dry_run {
            save_allow_scripts(&prefix, &remaining)?;
        }
    }

    if npm.json() {
        let payload = json!({
            "allowScripts": {
                "removed": removed.iter().map(|r| json!({ "key": r.key, "reason": r.reason })).collect::<Vec<_>>(),
                "dryRun": dry_run,
            }
        });
        crate::output::set_meta(true);
        crate::output::standard(payload.to_string());
        crate::output::flush();
        return Ok(());
    }
    if removed.is_empty() {
        crate::output::standard("No unused allowScripts entries.");
        return Ok(());
    }
    crate::output::standard(format!(
        "{} {} unused allowScripts {}:",
        if dry_run { "Would remove" } else { "Removed" },
        removed.len(),
        if removed.len() == 1 { "entry" } else { "entries" },
    ));
    for entry in &removed {
        let text = if entry.reason == "not-installed" {
            "package not installed"
        } else {
            "no install scripts"
        };
        crate::output::standard(format!("  {} ({text})", entry.key));
    }
    Ok(())
}
