//! Port of `lib/utils/explain-eresolve.js`: turns an ERESOLVE explanation
//! into a human-intelligible message plus a verbose report file.

use std::collections::BTreeSet;

use crate::utils::explain_dep::{explain_edge, explain_node, print_node, ExplainEdge, ExplainNode};

/// The explanation object Arborist attaches to an ERESOLVE error.
#[derive(Debug, Clone, Default)]
pub struct EresolveExplanation {
    /// Whether a node is being installed; Arborist only attaches these to
    /// nodes that are (`whileInstalling`).
    pub dep: Option<ExplainNode>,
    pub current: Option<ExplainNode>,
    pub peer_conflict: Option<PeerConflict>,
    pub current_edge: Option<ExplainEdge>,
    pub edge: Option<ExplainEdge>,
    pub strict_peer_deps: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PeerConflict {
    pub current: Option<ExplainNode>,
    pub peer: Option<ExplainNode>,
}

fn edge_text(edge: &ExplainEdge, depth: i32, color: bool) -> String {
    explain_edge(edge, depth, color, &mut BTreeSet::new())
}

/// `depth` is how far to descend; the report file uses `i32::MAX`.
pub fn explain(expl: &EresolveExplanation, color: bool, depth: i32) -> String {
    let while_installing = expl
        .dep
        .as_ref()
        .or(expl.current.as_ref())
        .or_else(|| expl.edge.as_ref().map(|edge| &edge.from));
    let mut out = Vec::new();
    if let Some(node) = while_installing {
        out.push(format!("While resolving: {}", print_node(node, color)));
    }

    let found = expl
        .current
        .as_ref()
        .map(|current| explain_node(current, depth, color))
        .or_else(|| {
            expl.peer_conflict
                .as_ref()
                .and_then(|pc| pc.current.as_ref())
                .map(|current| explain_node(current, depth, color))
        })
        .or_else(|| expl.current_edge.as_ref().map(|edge| edge_text(edge, depth, color)))
        .or_else(|| expl.edge.as_ref().map(|edge| edge_text(edge, depth, color)));
    if let Some(found) = found {
        out.push(format!("Found: {found}"));
    }

    let edge = expl
        .edge
        .as_ref()
        .map(|edge| edge_text(edge, depth, color))
        .unwrap_or_default();
    out.push(format!("\nCould not resolve dependency:\n{edge}"));

    if let Some(peer) = expl.peer_conflict.as_ref().and_then(|pc| pc.peer.as_ref()) {
        out.push(format!(
            "\nConflicting peer dependency: {}",
            explain_node(peer, depth, color)
        ));
    }
    out.join("\n")
}

fn or_join(items: &[&str]) -> String {
    match items.len() {
        0 => String::new(),
        1..=2 => items.join(" or "),
        _ => items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if i + 1 == items.len() {
                    format!("or {item}")
                } else {
                    (*item).to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Generates the full verbose report and the retry-with flags advice.
pub fn report(expl: &EresolveExplanation, color: bool, no_color: bool) -> (String, String) {
    let mut flags = Vec::new();
    if expl.strict_peer_deps {
        flags.push("--no-strict-peer-deps");
    }
    flags.push("--force");
    flags.push("--legacy-peer-deps");
    let fix = format!(
        "Fix the upstream dependency conflict, or retry this command with {} to accept an incorrect (and potentially broken) dependency resolution.",
        or_join(&flags)
    );
    let explanation = format!("{}\n\n{fix}", explain(expl, color, 4));
    let file = format!(
        "# npm resolution error report\n\n{}\n\n{fix}",
        explain(expl, no_color, i32::MAX)
    );
    (explanation, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn or_join_reads_naturally() {
        assert_eq!(or_join(&["a", "b"]), "a or b");
        assert_eq!(or_join(&["a", "b", "c"]), "a, b, or c");
    }

    #[test]
    fn report_lists_force_flags() {
        let (explanation, file) = report(&EresolveExplanation::default(), false, false);
        assert!(explanation.contains("--force or --legacy-peer-deps"));
        assert!(file.starts_with("# npm resolution error report"));
    }
}
