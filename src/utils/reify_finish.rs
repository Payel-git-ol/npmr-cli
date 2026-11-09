//! Port of `lib/utils/reify-finish.js` (minus the arborist dependency, which
//! is not ported yet): runs the reify pipeline and prints its summary.

use std::path::PathBuf;
use std::time::Instant;

use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::reify_output::{ReifySummary, reify_output};

#[derive(Debug, Default)]
pub struct ReifyOptions<'a> {
    pub path: PathBuf,
    pub audit: bool,
    pub fund: bool,
    pub save: Option<bool>,
    /// Packages to refresh; empty when updating the whole tree.
    pub update: &'a [String],
    pub add: &'a [String],
    pub remove: &'a [String],
    pub dry_run: bool,
    pub workspaces: &'a [String],
    /// `allowScripts` policy resolved from CLI/`.npmrc`/`package.json`.
    pub allow_scripts: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Reifies the tree at `options.path`. TODO: implement `@npmcli/arborist`.
pub fn reify(npm: &mut (dyn Npm + '_), options: ReifyOptions<'_>) -> Result<()> {
    let started = Instant::now();
    reify_tree(options.path.as_path())?;
    let summary = ReifySummary::default();
    reify_output(npm, &summary, started.elapsed());
    Ok(())
}

fn reify_tree(_path: &std::path::Path) -> Result<()> {
    Err(NpmError::new("Reify is not implemented yet").with_code("ENOTSUP"))
}

/// Re-reads the builtin config and writes it into a globally installed npm.
///
/// TODO: call after a global install once arborist is available.
pub fn preserve_builtin_config(_npm: &dyn Npm, _npm_node_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// Options shared by every command that hands a tree mutation to arborist.
#[derive(Debug, Clone)]
pub struct ArboristRun<'a> {
    pub prefix: std::path::PathBuf,
    pub add: &'a [String],
    pub remove: &'a [String],
    pub update: &'a [String],
    pub save: Option<bool>,
    pub dry_run: bool,
    pub workspaces: &'a [String],
    pub package_lock: Option<bool>,
}

impl<'a> ArboristRun<'a> {
    fn into_options(self, npm: &dyn Npm) -> ReifyOptions<'a> {
        let allow_scripts = crate::utils::resolve_allow_scripts::resolve_allow_scripts(npm, false)
            .ok()
            .and_then(|resolved| resolved.policy);
        ReifyOptions {
            path: self.prefix,
            audit: npm.store().get_bool("audit"),
            fund: npm.store().get_bool("fund"),
            save: self.save,
            update: self.update,
            add: self.add,
            remove: self.remove,
            dry_run: self.dry_run,
            workspaces: self.workspaces,
            allow_scripts,
        }
    }

    /// Resolves the allowlist policy and runs the reify pipeline.
    pub fn execute(self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let options = self.into_options(npm);
        reify(npm, options)
    }
}
