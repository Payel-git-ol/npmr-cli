//! Port of `lib/utils/get-workspaces.js`. Resolves `--workspace` filters
//! against the workspace map declared in the root `package.json`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

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
    let workspaces = map_workspaces(&root, &pkg)?;

    let mut res: BTreeMap<String, String> = BTreeMap::new();
    if input.include_workspace_root {
        let pkg_name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        res.insert(pkg_name, input.path.clone());
    }
    if input.filters.is_empty() {
        res.extend(workspaces);
        if res.is_empty() {
            return Err(NpmError::new("No workspaces found!"));
        }
        return Ok(res);
    }

    let relative_from = Path::new(&input.relative_from);
    // Computed once per workspace instead of once per filter.
    let workspaces: Vec<(String, String, String)> = workspaces
        .into_iter()
        .map(|(name, workspace_path)| {
            let relative_path = diff_paths(Path::new(&workspace_path), relative_from)
                .map(|p| slash_path(&p))
                .unwrap_or_default();
            (name, workspace_path, relative_path)
        })
        .collect();

    for filter in &input.filters {
        let relative_filter = diff_paths(Path::new(filter), &root)
            .map(|p| slash_path(&p))
            .unwrap_or_default();
        let mut prefixed = String::with_capacity(relative_filter.len() + 2);
        if filter.starts_with("./") {
            prefixed.push_str("./");
        }
        prefixed.push_str(&globify(&relative_filter));
        prefixed.push_str("/*");
        let under_filter = glob::Pattern::new(&prefixed).ok();
        let mut bare = String::with_capacity(filter.len() + 2);
        bare.push_str(&globify(filter));
        bare.push_str("/*");
        let under_bare = glob::Pattern::new(&bare).ok();
        let resolved_filter = resolve_join(relative_from, filter);

        for (name, workspace_path, relative_path) in &workspaces {
            if filter == name
                || resolved_filter.as_path() == Path::new(workspace_path)
                || under_filter
                    .as_ref()
                    .is_some_and(|p| p.matches_with(relative_path, glob::MatchOptions::new()))
                || under_bare
                    .as_ref()
                    .is_some_and(|p| p.matches_with(relative_path, glob::MatchOptions::new()))
            {
                res.insert(name.clone(), workspace_path.clone());
            }
        }
    }

    if res.is_empty() {
        let joined: String = input
            .filters
            .iter()
            .map(|f| format!(" --workspace={f}"))
            .collect();
        return Err(NpmError::new(format!("No workspaces found:\n {joined}")));
    }

    Ok(res)
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
    let patterns = workspace_patterns(pkg);

    let mut res = BTreeMap::new();
    if patterns.is_empty() {
        return Ok(res);
    }
    let mut root_slash = slash_path(root);
    root_slash.push('/');

    for pattern in patterns {
        let mut full = String::with_capacity(root_slash.len() + pattern.len());
        full.push_str(&root_slash);
        full.push_str(&globify(pattern));
        let Ok(paths) = glob::glob(&full) else {
            continue;
        };
        let mut matches: Vec<PathBuf> = paths.filter_map(|p| p.ok()).collect();
        matches.sort();
        for dir in matches {
            let Ok(manifest) = read_package_json(&dir) else {
                // No `package.json` here: not a workspace package.
                continue;
            };
            if let Some(name) = manifest.get("name").and_then(|v| v.as_str()) {
                res.insert(name.to_string(), slash_path(&dir));
            }
        }
    }
    Ok(res)
}

fn workspace_patterns(pkg: &serde_json::Value) -> Vec<&str> {
    let items = match pkg.get("workspaces") {
        Some(serde_json::Value::Array(items)) => items,
        Some(serde_json::Value::Object(map)) => match map.get("packages") {
            Some(serde_json::Value::Array(items)) => items,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    items.iter().filter_map(|v| v.as_str()).collect()
}

fn globify(pattern: &str) -> String {
    pattern.replace('\\', "/")
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
    let to = if to
        .components()
        .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        normalize(to)
    } else {
        to.to_path_buf()
    };
    let from = if from
        .components()
        .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        normalize(from)
    } else {
        from.to_path_buf()
    };
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
