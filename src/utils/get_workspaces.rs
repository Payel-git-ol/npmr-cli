//! Port of `lib/utils/get-workspaces.js`. Resolves `--workspace` filters
//! against the workspace map declared in the root `package.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::utils::errors::{NpmError, Result};

#[derive(Debug, Clone)]
pub struct GetWorkspacesInput {
    pub filters: Vec<String>,
    pub path: String,
    pub include_workspace_root: bool,
    pub relative_from: String,
}

/// Returns a map of workspace name -> absolute path.
pub fn get_workspaces(input: &GetWorkspacesInput) -> Result<BTreeMap<String, String>> {
    let root = PathBuf::from(&input.path);
    let pkg = read_package_json(&root)?;
    let pkg_name = pkg
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let workspaces = map_workspaces(&root, &pkg)?;

    let mut res: BTreeMap<String, String> = BTreeMap::new();
    if input.include_workspace_root {
        res.insert(pkg_name, input.path.clone());
    }
    if input.filters.is_empty() {
        res.extend(workspaces.clone());
    }

    let relative_from = Path::new(&input.relative_from);
    for filter in &input.filters {
        for (name, workspace_path) in &workspaces {
            let mut relative_path = diff_paths(Path::new(workspace_path), relative_from)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if filter.starts_with("./") {
                relative_path = format!("./{relative_path}");
            }
            let relative_filter = diff_paths(Path::new(filter), &root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();

            if filter == name
                || resolve_join(relative_from, filter) == Path::new(workspace_path)
                || glob_matches(&relative_path, &format!("{}/*", globify(&relative_filter)))
                || glob_matches(&relative_path, &format!("{}/*", globify(filter)))
            {
                res.insert(name.clone(), workspace_path.clone());
            }
        }
    }

    if res.is_empty() {
        let msg = if input.filters.is_empty() {
            "!".to_string()
        } else {
            let joined: String = input
                .filters
                .iter()
                .map(|f| format!(" --workspace={f}"))
                .collect();
            format!(":\n {joined}")
        };
        return Err(NpmError::new(format!("No workspaces found{msg}")));
    }

    Ok(res)
}

fn read_package_json(dir: &Path) -> Result<serde_json::Value> {
    let text = std::fs::read_to_string(dir.join("package.json"))
        .map_err(|_| NpmError::new("Invalid package.json"))?;
    serde_json::from_str(&text)
        .map_err(|_| NpmError::new("Invalid package.json").with_code("EJSONPARSE"))
}

/// Reads `workspaces` (array or `{packages}`) and expands the glob patterns to
/// package directories containing a `package.json`.
fn map_workspaces(root: &Path, pkg: &serde_json::Value) -> Result<BTreeMap<String, String>> {
    let mut patterns: Vec<String> = Vec::new();
    match pkg.get("workspaces") {
        Some(serde_json::Value::Array(items)) => {
            patterns.extend(items.iter().filter_map(|v| v.as_str()).map(String::from));
        }
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::Array(items)) = map.get("packages") {
                patterns.extend(items.iter().filter_map(|v| v.as_str()).map(String::from));
            }
        }
        _ => {}
    }

    let mut res = BTreeMap::new();
    for pattern in patterns {
        let full = format!(
            "{}/{}",
            root.to_string_lossy().replace('\\', "/"),
            globify(&pattern)
        );
        if let Ok(paths) = glob::glob(&full) {
            let mut matches: Vec<PathBuf> = paths.filter_map(|p| p.ok()).collect();
            matches.sort();
            for dir in matches {
                if !dir.join("package.json").is_file() {
                    continue;
                }
                if let Ok(manifest) = read_package_json(&dir) {
                    if let Some(name) = manifest.get("name").and_then(|v| v.as_str()) {
                        res.insert(name.to_string(), dir.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    Ok(res)
}

fn globify(pattern: &str) -> String {
    pattern.replace('\\', "/")
}

fn glob_matches(path: &str, pattern: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches_with(path, glob::MatchOptions::new()))
        .unwrap_or(false)
}

fn resolve_join(base: &Path, other: &str) -> PathBuf {
    let joined = base.join(other);
    normalize(&joined)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// `path.relative(from, to)` equivalent.
fn diff_paths(to: &Path, from: &Path) -> Option<PathBuf> {
    let to = normalize(to);
    let from = normalize(from);
    let mut to_it = to.components().peekable();
    let mut from_it = from.components().peekable();

    let mut common = 0;
    while let (Some(a), Some(b)) = (to_it.peek(), from_it.peek()) {
        if a == b {
            to_it.next();
            from_it.next();
            common += 1;
        } else {
            break;
        }
    }
    if common == 0 {
        return None;
    }

    let mut result = PathBuf::new();
    for _ in from_it {
        result.push("..");
    }
    for c in to_it {
        result.push(c.as_os_str());
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Some(result)
}
