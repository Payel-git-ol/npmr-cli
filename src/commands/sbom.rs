//! Port of `lib/commands/sbom.js`. The dependency selector and format
//! validation are ported; walking the tree needs arborist, so the command
//! reports `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::info;

pub static META: CommandMeta = CommandMeta::new(
    "sbom",
    "Generate a Software Bill of Materials (SBOM)",
)
.with_params(&[
    "omit",
    "package-lock-only",
    "sbom-format",
    "sbom-type",
    "workspace",
    "workspaces",
])
.workspaces();

pub const SBOM_FORMATS: &[&str] = &["cyclonedx", "spdx"];

/// Builds the dependency selector `npm sbom` queries the tree with.
pub fn build_selector(omit: &[String], workspaces_enabled: bool, workspace_names: &[String]) -> String {
    let omits: String = omit.iter().map(|o| format!(":not(.{o})")).collect();
    let selector = if !workspaces_enabled {
        format!(":root > :not(.workspace){omits},:root > :not(.workspace) *{omits},:extraneous")
    } else if !workspace_names.is_empty() {
        workspace_names
            .iter()
            .map(|ws| format!("#{ws},#{ws} *{omits}"))
            .collect::<Vec<_>>()
            .join(",")
    } else {
        format!(":root *{omits},:extraneous")
    };
    format!(":root,{selector}")
}

pub struct Sbom;

impl Sbom {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sbom {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Sbom {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let format = npm.store().get_str("sbom-format");
        if !SBOM_FORMATS.contains(&format.as_str()) {
            return Err(META.usage_error(format!(
                "Must specify --sbom-format flag with one of: {}.",
                SBOM_FORMATS.join(", ")
            )));
        }
        let package_lock_only = npm.store().get_bool("package-lock-only");
        let omit: Vec<String> = npm
            .flat_options()
            .get("omit")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let selector = build_selector(&omit, true, &[]);
        info("sbom", &[&format!("Using dependency selector: {selector}")]);
        let _workspaces = args;
        Err(NpmError::new(format!(
            "Cannot generate a {format} SBOM{}: loading the install tree is not implemented yet",
            if package_lock_only {
                " from package-lock.json"
            } else {
                ""
            }
        ))
        .with_code("ENOTSUP"))
    }
}

#[cfg(test)]
mod tests {
    use super::build_selector;

    #[test]
    fn selector_includes_root_and_omits() {
        assert_eq!(
            build_selector(&["dev".into()], true, &[]),
            ":root,:root *:not(.dev),:extraneous"
        );
    }
}
