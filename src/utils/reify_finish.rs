//! Port of `lib/utils/reify-finish.js` (minus the arborist dependency, which
//! is not ported yet): runs the reify pipeline and prints its summary.

use std::path::PathBuf;
use std::time::Instant;

use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::reify_output::{ReifySummary, reify_output};

#[derive(Debug, Clone, Default)]
pub struct ReifyOptions {
    pub path: PathBuf,
    pub audit: bool,
    pub fund: bool,
    pub save: Option<bool>,
    pub update: Vec<String>,
}

/// Reifies the tree at `options.path`. TODO: implement `@npmcli/arborist`.
pub fn reify(npm: &mut (dyn Npm + '_), options: ReifyOptions) -> Result<()> {
    let started = Instant::now();
    reify_tree(options)?;
    let summary = ReifySummary::default();
    reify_output(npm, &summary, started.elapsed());
    Ok(())
}

fn reify_tree(_options: ReifyOptions) -> Result<()> {
    Err(NpmError::new("Reify is not implemented yet").with_code("ENOTSUP"))
}

/// Re-reads the builtin config and writes it into a globally installed npm.
///
/// TODO: call after a global install once arborist is available.
pub fn preserve_builtin_config(_npm: &dyn Npm, _npm_node_path: &std::path::Path) -> Result<()> {
    Ok(())
}
