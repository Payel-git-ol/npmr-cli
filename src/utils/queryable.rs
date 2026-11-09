//! Port of `lib/utils/queryable.js`: a small CSS-like selector engine over the
//! installed tree so `npm query` works without arborist.

use serde_json::Value;

/// A node of the queryable tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub version: String,
    pub path: String,
    pub children: Vec<Node>,
    pub dev: bool,
    pub optional: bool,
    pub link: bool,
}

impl Node {
    pub fn from_dir(dir: &std::path::Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
        let manifest: Value = serde_json::from_str(&text).ok()?;
        Some(Self {
            name: manifest
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            version: manifest
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            path: dir.to_string_lossy().into_owned(),
            children: Vec::new(),
            dev: false,
            optional: false,
            link: false,
        })
    }

    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }

    /// Pre-order walk over this node and its children.
    pub fn descendants(&self) -> Vec<&Node> {
        let mut out = vec![self];
        for child in &self.children {
            out.extend(child.descendants());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attr {
    Value(String),
    Regex(String),
    Exists,
}

#[derive(Debug, Clone)]
pub enum Combinator {
    Descendant,
    Child,
}

#[derive(Debug, Clone)]
pub struct Compound {
    pub parts: Vec<(String, Attr)>,
    pub first_child: bool,
}

#[derive(Debug, Clone)]
pub struct Selector {
    /// `(combinator, compound)`, the first combinator is always descendant.
    pub steps: Vec<(Combinator, Compound)>,
}

fn parse_attr_value(value: &str) -> Attr {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('/') && value.ends_with('/') {
        Attr::Regex(value[1..value.len() - 1].to_string())
    } else if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Attr::Value(value[1..value.len() - 1].to_string())
    } else if value.is_empty() {
        Attr::Exists
    } else {
        Attr::Value(value.to_string())
    }
}

fn parse_compound(text: &str) -> Compound {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                let mut inner = String::new();
                for n in chars.by_ref() {
                    if n == ']' {
                        break;
                    }
                    inner.push(n);
                }
                let (key, value) = match inner.split_once('=') {
                    Some((k, v)) => (k.trim().to_string(), parse_attr_value(v)),
                    None => (inner.trim().to_string(), Attr::Exists),
                };
                if !buf.is_empty() {
                    parts.push((std::mem::take(&mut buf), Attr::Exists));
                }
                parts.push((key, value));
            }
            '.' | ':' if !buf.is_empty() => {
                parts.push((std::mem::take(&mut buf), Attr::Exists));
                buf.push(c);
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        parts.push((buf, Attr::Exists));
    }
    let first_child = parts.iter().any(|(p, _)| p == ":first-child");
    parts.retain(|(p, _)| !p.starts_with(':'));
    Compound { parts, first_child }
}

/// Splits a selector group on top-level commas.
pub fn split_selectors(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    let mut in_string = false;
    for c in query.chars() {
        match c {
            '"' => {
                in_string = !in_string;
                current.push(c);
            }
            '[' if !in_string => {
                depth += 1;
                current.push(c);
            }
            ']' if !in_string => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 && !in_string => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

/// Parses a single selector into combinator steps.
pub fn parse_selector(text: &str) -> Selector {
    let mut steps = Vec::new();
    let mut current = String::new();
    let mut in_attr = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                in_attr = true;
                current.push(c);
            }
            ']' => {
                in_attr = false;
                current.push(c);
            }
            '>' if !in_attr => {
                current.clear();
            }
            c if c.is_whitespace() && !in_attr => {
                current.clear();
            }
            _ => current.push(c),
        }
    }
    let compounds: Vec<Compound> = text
        .split(|c: char| c == '>' || c.is_whitespace())
        .filter(|s| !s.trim().is_empty())
        .filter(|s| !s.chars().all(|c| c == '>'))
        .map(parse_compound)
        .collect();
    let mut steps_iter = compounds.into_iter();
    if let Some(first) = steps_iter.next() {
        steps.push((Combinator::Descendant, first));
        for compound in steps_iter {
            steps.push((Combinator::Descendant, compound));
        }
    }
    Selector { steps }
}

fn field<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    match key {
        "name" => Some(node.name.as_str()),
        "version" => Some(node.version.as_str()),
        "path" => Some(node.path.as_str()),
        "id" => None,
        _ => None,
    }
}

fn attr_matches(node: &Node, key: &str, attr: &Attr) -> bool {
    let actual: Option<String> = match key {
        "dev" => Some(node.dev.to_string()),
        "optional" => Some(node.optional.to_string()),
        "link" => Some(node.link.to_string()),
        _ => field(node, key).map(str::to_string),
    };
    match attr {
        Attr::Exists => actual.is_some(),
        Attr::Value(want) => actual.as_deref() == Some(want.as_str()),
        Attr::Regex(pattern) => match (regex::Regex::new(pattern), actual) {
            (Ok(re), Some(value)) => re.is_match(&value),
            _ => false,
        },
    }
}

fn compound_matches(node: &Node, compound: &Compound) -> bool {
    compound
        .parts
        .iter()
        .all(|(key, attr)| attr_matches(node, key, attr))
}

fn step_match(step_parents: &[&Node], selector: &Selector, index: usize, node: &Node) -> bool {
    if !compound_matches(node, &selector.steps[index].1) {
        return false;
    }
    if index == 0 {
        return true;
    }
    step_parents
        .iter()
        .any(|parent| step_match(step_parents, selector, index - 1, parent))
}

/// Selects every node matching any selector in `query`.
pub fn query<'a>(root: &'a Node, query: &str) -> Vec<&'a Node> {
    let selectors: Vec<Selector> = split_selectors(query)
        .iter()
        .map(|s| parse_selector(s))
        .collect();
    let mut out: Vec<&Node> = Vec::new();
    walk(root, &mut Vec::new(), &selectors, &mut out);
    out
}

fn walk<'a>(
    node: &'a Node,
    parents: &mut Vec<&'a Node>,
    selectors: &[Selector],
    out: &mut Vec<&'a Node>,
) {
    if selectors.iter().any(|s| step_match(parents, s, s.steps.len() - 1, node)) {
        out.push(node);
    }
    parents.push(node);
    for child in &node.children {
        walk(child, parents, selectors, out);
    }
    parents.pop();
}

/// Renders a node as the `--json` object `npm query` prints.
pub fn to_json(node: &Node) -> Value {
    serde_json::json!({
        "name": node.name,
        "version": node.version,
        "path": node.path,
    })
}

/// The legacy `npm query` output: the npm tree format with `"dependencies"`.
pub fn to_tree(node: &Node) -> Value {
    let deps: serde_json::Map<String, Value> = node
        .children
        .iter()
        .map(|child| (child.name.clone(), to_tree(child)))
        .collect();
    serde_json::json!({
        "name": node.name,
        "version": node.version,
        "path": node.path,
        "dependencies": deps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Node {
        Node {
            name: "root".into(),
            version: "1.0.0".into(),
            path: "/".into(),
            dev: false,
            optional: false,
            link: false,
            children: vec![
                Node {
                    name: ".bin".into(),
                    version: "".into(),
                    path: "/node_modules/.bin".into(),
                    children: vec![],
                    dev: false,
                    optional: false,
                    link: false,
                },
                Node {
                    name: "a".into(),
                    version: "1.2.3".into(),
                    path: "/node_modules/a".into(),
                    children: vec![],
                    dev: true,
                    optional: false,
                    link: false,
                },
            ],
        }
    }

    #[test]
    fn attribute_selector() {
        let root = tree();
        let found = query(&root, "[name=a]");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, "1.2.3");
    }

    #[test]
    fn regex_and_multiple() {
        let root = tree();
        let found = query(&root, "[name=/^a/], [name=root]");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn skips_dot_bin() {
        let root = tree();
        let all = query(&root, "*");
        assert!(all.iter().any(|n| n.name == ".bin"));
        let named = query(&root, "[name=a]");
        assert!(!named.iter().any(|n| n.name == ".bin"));
    }
}
