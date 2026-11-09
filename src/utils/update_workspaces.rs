//! Port of `lib/utils/update-workspaces.js`: re-runs a minimal reify after a
//! command changed workspace manifests (e.g. `npm version -ws`).

use crate::npm::Npm;
use crate::utils::errors::Result;

#[derive(Debug, Clone)]
pub struct UpdateWorkspacesInput {
    pub local_prefix: std::path::PathBuf,
    pub workspaces: Vec<String>,
}

pub fn update_workspaces(npm: &mut (dyn Npm + '_), input: &UpdateWorkspacesInput) -> Result<()> {
    let store = npm.store();
    if !store.get_bool("workspaces-update") || input.workspaces.is_empty() {
        return Ok(());
    }
    // Default is not to save, to avoid racing when publishing interdependent
    // workspaces; `--save` opts back in.
    let save = if store.is_default("save") {
        Some(false)
    } else {
        Some(store.get_bool("save"))
    };
    let options = crate::utils::reify_finish::ReifyOptions {
        path: input.local_prefix.clone(),
        audit: false,
        fund: false,
        save,
        update: input.workspaces.clone(),
        ..Default::default()
    };
    crate::utils::reify_finish::reify(npm, options)
}
