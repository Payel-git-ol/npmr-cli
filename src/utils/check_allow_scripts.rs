//! Thin wrapper around the shared unreviewed-scripts walk, ported from
//! `check-allow-scripts.js`.

use crate::utils::allow_scripts_writer::{Policy, ScriptNode};

pub struct UnreviewedEntry<'a> {
    pub node: &'a ScriptNode,
    pub scripts: Vec<(String, String)>,
}

/// Returns the dependencies that declare install scripts and are neither
/// allowed nor denied by `policy`. While arborist is not ported the caller
/// supplies the candidate nodes and their scripts.
pub fn check_allow_scripts<'a>(
    nodes: &'a [ScriptNode],
    scripts: impl Fn(&ScriptNode) -> Vec<(String, String)>,
    policy: Option<&Policy>,
    ignore_scripts: bool,
    include_when_ignored: bool,
) -> Vec<UnreviewedEntry<'a>> {
    if ignore_scripts && !include_when_ignored {
        return Vec::new();
    }
    nodes
        .iter()
        .filter_map(|node| {
            let scripts = scripts(node);
            if scripts.is_empty() || covered(node, policy) {
                return None;
            }
            Some(UnreviewedEntry { node, scripts })
        })
        .collect()
}

/// An install script is covered when any policy key targets the node.
fn covered(node: &ScriptNode, policy: Option<&Policy>) -> bool {
    let Some(policy) = policy else {
        return false;
    };
    policy.keys().any(|key| {
        crate::utils::allow_scripts_writer::parse_key(key).is_some_and(|parsed| {
            crate::utils::allow_scripts_writer::key_targets_node(&parsed, node)
        })
    })
}
