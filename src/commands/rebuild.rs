//! Port of `lib/commands/rebuild.js`. Spec validation and bundled-dependency
//! rules are ported; rebuilding needs the arborist tree, so it reports
//! `ENOTSUP`.

use std::path::{Path, PathBuf};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("rebuild", "Rebuild a package")
    .with_params(&[
        "global",
        "bin-links",
        "foreground-scripts",
        "ignore-scripts",
        "allow-scripts",
        "strict-allow-scripts",
        "dangerously-allow-all-scripts",
        "workspace",
        "workspaces",
        "include-workspace-root",
    ])
    .with_usage(&["[<package-spec>] ...]"])
    .arborist();

/// A parsed `npm rebuild` target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildSpec {
    pub name: String,
    /// `None` means "any version".
    pub spec: Option<String>,
    /// Directory specs match by path instead of name.
    pub directory: Option<PathBuf>,
}

impl RebuildSpec {
    /// Bundled dependencies are never selected by name.
    pub fn matches(&self, name: &str, version: &str, path: &Path, in_bundle: bool) -> bool {
        if in_bundle {
            return false;
        }
        if let Some(directory) = &self.directory {
            return path == directory;
        }
        if self.name != name {
            return false;
        }
        match &self.spec {
            None => true,
            Some(spec) => semver::VersionReq::parse(spec)
                .ok()
                .is_some_and(|req| {
                    semver::Version::parse(version).is_ok_and(|v| req.matches(&v))
                }),
        }
    }
}

/// Parses one `npm rebuild` argument.
pub fn parse_rebuild_spec(arg: &str) -> Result<RebuildSpec> {
    if arg.starts_with('.') || arg.starts_with('/') || arg.starts_with("file:") {
        let path = arg.trim_start_matches("file:");
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
        return Ok(RebuildSpec {
            name: String::new(),
            spec: None,
            directory: Some(path),
        });
    }
    if arg.contains("://") || arg.starts_with("git") || arg.contains(':') {
        return Err(NpmError::new(
            "`npm rebuild` only supports SemVer version/range specifiers",
        ));
    }
    let (name, spec) = match arg.rfind('@') {
        Some(0) | None => (arg.to_string(), None),
        Some(idx) => {
            let spec = &arg[idx + 1..];
            let spec = (!spec.is_empty() && spec != "*").then(|| spec.to_string());
            (arg[..idx].to_string(), spec)
        }
    };
    if let Some(spec) = &spec
        && semver::VersionReq::parse(spec).is_err()
        && semver::Version::parse(spec).is_err()
    {
        return Err(NpmError::new(
            "`npm rebuild` only supports SemVer version/range specifiers",
        ));
    }
    Ok(RebuildSpec {
        name,
        spec,
        directory: None,
    })
}

pub struct Rebuild;

impl Rebuild {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Rebuild {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Rebuild {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let specs = args
            .iter()
            .map(|arg| parse_rebuild_spec(arg))
            .collect::<Result<Vec<_>>>()?;
        let _where = if npm.global() {
            let dir = npm.global_dir();
            dir.parent().unwrap_or(&dir).to_path_buf()
        } else {
            npm.prefix()
        };
        let _ = crate::utils::resolve_allow_scripts::resolve_allow_scripts(&*npm, false)?;
        let _ = specs;
        Err(NpmError::new(
            "npm rebuild requires the arborist tree, which is not implemented yet",
        )
        .with_code("ENOTSUP"))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_rebuild_spec, RebuildSpec};

    #[test]
    fn parses_names_and_ranges() {
        assert_eq!(
            parse_rebuild_spec("canvas").expect("valid spec"),
            RebuildSpec {
                name: "canvas".into(),
                spec: None,
                directory: None,
            }
        );
        assert_eq!(
            parse_rebuild_spec("canvas@^2").expect("valid spec").spec,
            Some("^2".into())
        );
        assert!(parse_rebuild_spec("git+https://example.com/x.git").is_err());
    }

    #[test]
    fn bundled_nodes_never_match_by_name() {
        let spec = parse_rebuild_spec("bcrypt").expect("valid spec");
        assert!(!spec.matches("bcrypt", "1.0.0", std::path::Path::new("/x"), true));
        assert!(spec.matches("bcrypt", "1.0.0", std::path::Path::new("/x"), false));
    }
}
