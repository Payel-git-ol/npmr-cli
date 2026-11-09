//! Port of `lib/utils/warn-workspace-allow-scripts.js`. The `allowScripts`
//! policy must live at the project root, so a workspace declaring its own is
//! almost always a mistake worth warning about.

use crate::utils::log::warn;

/// A workspace that declares its own `allowScripts`.
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
}

fn workspace_patterns(root: &std::path::Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let items = match manifest.get("workspaces") {
        Some(serde_json::Value::Array(items)) => items,
        Some(serde_json::Value::Object(map)) => match map.get("packages") {
            Some(serde_json::Value::Array(items)) => items,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

pub fn find_workspace_allow_scripts(root: &std::path::Path) -> Vec<WorkspacePackage> {
    let mut offenders = Vec::new();
    for pattern in workspace_patterns(root) {
        let Ok(matches) = glob::glob(&root.join(pattern).to_string_lossy()) else {
            continue;
        };
        for dir in matches.flatten() {
            let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
                continue;
            };
            let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if pkg.get("allowScripts").is_none() {
                continue;
            }
            let name = pkg
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| dir.to_str().unwrap_or_default())
                .to_string();
            offenders.push(WorkspacePackage {
                name,
                path: dir.display().to_string(),
            });
        }
    }
    offenders
}

pub fn warn_workspace_allow_scripts(root: &std::path::Path) {
    for workspace in find_workspace_allow_scripts(root) {
        warn(
            "install-scripts",
            &[&format!(
                "allowScripts in workspace {} ({}) is ignored. Move the field to the project root package.json.",
                workspace.name, workspace.path
            )],
        );
    }
}
