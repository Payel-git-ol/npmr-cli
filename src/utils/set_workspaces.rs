//! Port of `BaseCommand#setWorkspaces`: resolves the workspace map for the
//! current prefix.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::get_workspaces::{GetWorkspacesInput, get_workspaces};

pub fn set_workspaces(npm: &(dyn Npm + '_)) -> Result<BTreeMap<String, String>> {
    let prefix: PathBuf = npm.local_prefix().to_path_buf();
    let include_root = npm.store().get_bool("include-workspace-root");
    get_workspaces(&GetWorkspacesInput {
        filters: npm.store().get_list("workspace"),
        path: prefix.to_string_lossy().into_owned(),
        include_workspace_root: include_root,
        relative_from: prefix.to_string_lossy().into_owned(),
    })
}
