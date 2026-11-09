//! Port of `lib/utils/explain-dep.js`: renders an explanation node as the
//! multi-line tree `npm explain` prints.

use std::collections::BTreeSet;

use crate::utils::display::{paint, Color};

/// A dependency edge that pulled a node in.
#[derive(Debug, Clone, Default)]
pub struct ExplainEdge {
    pub name: String,
    /// `prod`, `dev`, `optional`, `peer`, `workspace`, ...
    pub edge_type: String,
    pub bundled: bool,
    pub spec: String,
    pub raw_spec: String,
    pub overridden: bool,
    pub from: ExplainNode,
}

/// The subset of an arborist node the explanation renderer reads.
#[derive(Debug, Clone, Default)]
pub struct ExplainNode {
    pub name: String,
    pub version: String,
    pub location: Option<String>,
    pub extraneous: bool,
    pub dev: bool,
    pub optional: bool,
    pub dev_optional: bool,
    pub peer: bool,
    pub bundled: bool,
    pub workspace: bool,
    pub overridden: bool,
    pub dependents: Vec<ExplainEdge>,
    pub links_in: Vec<ExplainNode>,
}

impl ExplainNode {
    fn type_label(&self, label: &'static str) -> Option<&'static str> {
        let flagged = match label {
            "extraneous" => self.extraneous,
            "dev" => self.dev,
            "optional" => self.optional,
            "peer" => self.peer,
            "bundled" => self.bundled,
            "workspace" => self.workspace,
            "overridden" => self.overridden,
            "devOptional" => self.dev_optional,
            _ => false,
        };
        flagged.then_some(label)
    }
}

fn color_type(label: &str, color: bool) -> String {
    let style = match label {
        "extraneous" => Color::Red,
        "dev" => Color::Blue,
        "optional" => Color::Magenta,
        "peer" => Color::MagentaBright,
        "bundled" => Color::Cyan,
        "workspace" => Color::BlueBright,
        "overridden" => Color::White,
        _ => return label.to_string(),
    };
    paint(label, style, color)
}

fn indent(text: &str) -> String {
    text.replace('\n', "\n  ")
}

/// The package name and version, plus location and type flags.
pub fn print_node(node: &ExplainNode, color: bool) -> String {
    let mut extra = Vec::new();
    for meta in ["extraneous", "dev", "optional", "peer", "bundled", "overridden"] {
        if let Some(label) = node.type_label(meta) {
            extra.push(format!(" {}", color_type(label, color)));
        }
    }
    let pkgid = format!("{}@{}", node.name, node.version);
    let pkgid = if node.workspace {
        paint(&pkgid, Color::BlueBright, color)
    } else {
        pkgid
    };
    let location = match &node.location {
        Some(location) => paint(&format!("\n{location}"), Color::White, color),
        None => String::new(),
    };
    format!("{pkgid}{}{location}", extra.join(""))
}

fn explain_links_in(node: &ExplainNode, depth: i32, color: bool, seen: &mut BTreeSet<String>) -> String {
    if node.links_in.is_empty() || depth <= 0 {
        return String::new();
    }
    let messages: Vec<String> = node
        .links_in
        .iter()
        .map(|link| explain_node_inner(link, depth - 1, color, seen))
        .collect();
    indent(&format!("\n{}", messages.join("\n")))
}

fn explain_dependents(node: &ExplainNode, depth: i32, color: bool, seen: &mut BTreeSet<String>) -> String {
    if node.dependents.is_empty() || depth <= 0 {
        return String::new();
    }
    let max = (depth as usize).div_ceil(2).max(1);
    let mut messages: Vec<String> = node
        .dependents
        .iter()
        .take(max)
        .map(|edge| explain_edge(edge, depth, color, seen))
        .collect();

    if node.dependents.len() > max {
        let mut len = 0usize;
        let mut names = Vec::new();
        for (i, edge) in node.dependents.iter().enumerate().skip(max) {
            let dep_name = if edge.from.name.is_empty() {
                "the root project"
            } else {
                edge.from.name.as_str()
            };
            len += dep_name.len();
            if len >= 50 && i < node.dependents.len() - 1 {
                names.push("...");
                break;
            }
            names.push(dep_name);
        }
        messages.push(format!(
            "{} more ({})",
            node.dependents.len() - max,
            names.join(", ")
        ));
    }
    indent(&format!("\n{}", messages.join("\n")))
}

/// Renders an edge as `type dep@"spec" from ...`.
pub fn explain_edge(edge: &ExplainEdge, depth: i32, color: bool, seen: &mut BTreeSet<String>) -> String {
    let mut dep = format!("{}@\"{}\"", edge.name, edge.spec);
    if edge.overridden {
        dep = format!("{} {dep} (was \"{}\")", color_type("overridden", color), edge.raw_spec);
    }
    let from = explain_from(&edge.from, depth, color, seen);
    let type_prefix = if edge.edge_type == "prod" {
        String::new()
    } else {
        format!("{} ", color_type(&edge.edge_type, color))
    };
    let bundled = if edge.bundled {
        format!("{} ", color_type("bundled", color))
    } else {
        String::new()
    };
    format!("{type_prefix}{bundled}{dep} from {from}")
}

fn explain_from(from: &ExplainNode, depth: i32, color: bool, seen: &mut BTreeSet<String>) -> String {
    if from.name.is_empty() && from.version.is_empty() {
        return "the root project".to_string();
    }
    // Track the current recursion stack only: diamonds are explained twice,
    // cycles are cut.
    let id = format!(
        "{}@{}:{}",
        from.name,
        from.version,
        from.location.as_deref().unwrap_or("")
    );
    if !seen.insert(id.clone()) {
        return print_node(from, color);
    }
    let result = format!(
        "{}{}{}",
        print_node(from, color),
        explain_dependents(from, depth - 1, color, seen),
        explain_links_in(from, depth - 1, color, seen)
    );
    seen.remove(&id);
    result
}

/// Renders a node with unlimited depth, as `npm explain` does.
pub fn explain_node(node: &ExplainNode, depth: i32, color: bool) -> String {
    let mut seen = BTreeSet::new();
    explain_node_inner(node, depth, color, &mut seen)
}

fn explain_node_inner(node: &ExplainNode, depth: i32, color: bool, seen: &mut BTreeSet<String>) -> String {
    format!(
        "{}{}{}",
        print_node(node, color),
        explain_dependents(node, depth, color, seen),
        explain_links_in(node, depth, color, seen)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_root_and_dependents() {
        let root = ExplainNode::default();
        let node = ExplainNode {
            name: "canvas".into(),
            version: "1.0.0".into(),
            location: Some("node_modules/canvas".into()),
            dev: true,
            dependents: vec![ExplainEdge {
                name: "canvas".into(),
                edge_type: "prod".into(),
                spec: "^1.0.0".into(),
                from: root,
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = explain_node(&node, i32::MAX, false);
        assert!(text.starts_with("canvas@1.0.0 dev\nnode_modules/canvas"));
        assert!(text.contains("canvas@\"^1.0.0\" from the root project"));
    }
}
