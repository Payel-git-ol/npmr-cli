//! Port of `lib/commands/edit.js`: opens an installed package in `$EDITOR`
//! then rebuilds it.

use std::path::PathBuf;

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("edit", "Edit an installed package")
    .with_params(&["editor"])
    .with_usage(&["<pkg>[/<subpkg>...]"]);

pub struct Edit;

impl Edit {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Edit {
    fn default() -> Self {
        Self::new()
    }
}

/// `a/node_modules/b` for `a/b`, keeping scoped names intact.
pub fn split_package_names(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for part in path.split('/') {
        match parts.last_mut() {
            Some(last) if last.starts_with('@') && !last.contains('/') => {
                last.push('/');
                last.push_str(part);
            }
            _ => parts.push(part.to_string()),
        }
    }
    parts.join("/node_modules/")
}

impl Command for Edit {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.len() != 1 {
            return Err(META.usage_error(""));
        }
        let dir: PathBuf = npm.dir().join(split_package_names(&args[0]));
        if !dir.exists() {
            return Err(NpmError::new(format!("no such package: {}", dir.display()))
                .with_code("ENOENT"));
        }

        let editor = npm.store().get_str("editor");
        let mut words = editor.split_whitespace();
        let Some(bin) = words.next() else {
            return Err(NpmError::new("No editor configured, set the `editor` config")
                .with_code("ENOEDITOR"));
        };
        let status = std::process::Command::new(bin)
            .args(words)
            .arg(&dir)
            .status()
            .map_err(NpmError::from)?;
        if !status.success() {
            let code = status.code().unwrap_or(1);
            return Err(NpmError::new(format!("editor process exited with code: {code}")));
        }

        npm.exec("rebuild", &[dir.to_string_lossy().into_owned()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_scopes_together() {
        assert_eq!(split_package_names("a/b"), "a/node_modules/b");
        assert_eq!(split_package_names("@sc/pkg"), "@sc/pkg");
        assert_eq!(
            split_package_names("@sc/pkg/sub"),
            "@sc/pkg/node_modules/sub"
        );
    }
}
