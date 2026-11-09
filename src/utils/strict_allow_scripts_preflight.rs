//! Port of `lib/utils/strict-allow-scripts-preflight.js`: with
//! `--strict-allow-scripts`, refuses to install when any package's install
//! scripts are not covered by the `allowScripts` policy.

use crate::utils::allow_scripts_remediation::config_set_allow_scripts;
use crate::utils::allow_scripts_writer::{Policy, ScriptNode};
use crate::utils::check_allow_scripts::check_allow_scripts;
use crate::utils::errors::{NpmError, Result};

/// Renders the pre-flight error, or `Ok(())` when nothing is unreviewed.
/// `scripts_of` reports the install scripts of each candidate node.
pub fn strict_allow_scripts_preflight(
    global: bool,
    policy: Option<&Policy>,
    ignore_scripts: bool,
    nodes: &[ScriptNode],
    scripts_of: impl Fn(&ScriptNode) -> Vec<(String, String)>,
) -> Result<()> {
    let unreviewed = check_allow_scripts(nodes, scripts_of, policy, ignore_scripts, false);
    if unreviewed.is_empty() {
        return Ok(());
    }

    let lines = unreviewed
        .iter()
        .map(|entry| {
            let events = entry
                .scripts
                .iter()
                .map(|(event, body)| format!("{event}: {body}"))
                .collect::<Vec<_>>()
                .join("; ");
            let label = if entry.node.version.is_empty() {
                entry.node.name.clone()
            } else {
                format!("{}@{}", entry.node.name, entry.node.version)
            };
            format!("  {label} ({events})")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let remediation = if global {
        let names: Vec<String> = unreviewed
            .iter()
            .map(|entry| entry.node.name.clone())
            .collect();
        format!(
            "Allow them with `--allow-scripts`, persist them with `{}`, or bypass this check with `--dangerously-allow-all-scripts`.",
            config_set_allow_scripts(&names)
        )
    } else {
        "Approve them with `npm install-scripts approve`, deny them with `npm install-scripts deny`, or bypass this check with `--dangerously-allow-all-scripts`."
            .to_string()
    };

    Err(NpmError::new(format!(
        "--strict-allow-scripts: {} package(s) have install scripts not covered by allowScripts:\n{lines}\n{remediation}",
        unreviewed.len()
    ))
    .with_code("ESTRICTALLOWSCRIPTS"))
}
