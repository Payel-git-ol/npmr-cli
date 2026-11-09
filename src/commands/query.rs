//! Port of `lib/commands/query.js`.

use std::path::Path;

use serde_json::Value;

use crate::commands::arborist_cmd::WorkspaceContext;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::{NpmError, Result};
use crate::utils::queryable::{self, Node};

pub static META: CommandMeta = CommandMeta::new("query", "Retrieve a filtered list of packages")
    .with_params(&[
        "global",
        "workspace",
        "workspaces",
        "include-workspace-root",
        "package-lock-only",
        "expect-results",
        "before",
        "min-release-age",
        "min-release-age-exclude",
    ])
    .with_usage(&["<selector>"])
    .workspaces();

pub struct Query {
    response: Vec<Value>,
}

impl Query {
    pub fn new() -> Self {
        Self {
            response: Vec::new(),
        }
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the queryable tree for `root` by reading `node_modules`.
pub fn load_tree(root: &Path) -> Result<Node> {
    let mut node = Node::from_dir(root)
        .ok_or_else(|| NpmError::new(format!("no package.json at {}", root.display())))?;
    node.children = read_children(&root.join("node_modules"));
    Ok(node)
}

fn read_children(modules: &Path) -> Vec<Node> {
    let Ok(entries) = std::fs::read_dir(modules) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    let mut children = Vec::new();
    for name in names {
        let dir = modules.join(&name);
        if name.starts_with('@') {
            let Ok(scoped) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut subs: Vec<String> = scoped
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            subs.sort();
            for sub in subs {
                if let Some(mut child) = Node::from_dir(&dir.join(&sub)) {
                    child.children = read_children(&dir.join(&sub).join("node_modules"));
                    children.push(child);
                }
            }
        } else if let Some(mut child) = Node::from_dir(&dir) {
            child.children = read_children(&dir.join("node_modules"));
            children.push(child);
        }
    }
    children
}

impl Query {
    fn run_query(&mut self, root: &Path, selector: &str) -> Result<()> {
        let tree = load_tree(root)?;
        for node in queryable::query(&tree, selector) {
            let value = queryable::to_json(node);
            if !self
                .response
                .iter()
                .any(|v| v.get("path") == value.get("path"))
            {
                self.response.push(value);
            }
        }
        Ok(())
    }

    fn check_expected(&self, npm: &dyn Npm) -> Result<()> {
        let Some(expected) = npm
            .store()
            .get("expect-results")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Ok(());
        };
        let actual = self.response.len();
        if expected.parse::<usize>().ok() == Some(actual) {
            return Ok(());
        }
        Err(
            NpmError::new(format!("Expected {expected} results, but found {actual}"))
                .with_code("EEXPECTRESULTS"),
        )
    }

    fn output(&self, npm: &dyn Npm) -> Result<()> {
        self.check_expected(npm)?;
        crate::output::set_meta(true);
        let value = Value::Array(self.response.clone());
        crate::output::standard(serde_json::to_string_pretty(&value).unwrap_or_default());
        crate::output::flush();
        Ok(())
    }
}

impl Command for Query {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        npm.store_mut()
            .set(Value::Bool(true), "json", ConfigSource::Cli);
        let selector = args.first().map(String::as_str).unwrap_or("*");
        let dir = npm.dir();
        let root = dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| npm.prefix());
        if npm.store().get_bool("package-lock-only") && !root.join("package-lock.json").is_file() {
            return Err(
                META.usage_error("A package-lock.json file is required in package-lock-only mode")
            );
        }
        self.run_query(&root, selector)?;
        self.output(npm)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let selector = args.first().map(String::as_str).unwrap_or("*").to_string();
        npm.store_mut()
            .set(Value::Bool(true), "json", ConfigSource::Cli);
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let ctx = WorkspaceContext::new(npm, workspaces.clone());
        if ctx.store().get_bool("include-workspace-root") {
            self.run_query(&ctx.prefix(), &selector)?;
        }
        for path in workspaces.values() {
            self.run_query(Path::new(path), &selector)?;
        }
        self.output(&ctx)
    }
}
