//! Port of `lib/commands/deprecate.js` and `undeprecate.js`. Writing the
//! packument back to the registry needs `npm-registry-fetch`, which is still
//! stubbed, so the local validation runs first and the request reports
//! `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("deprecate", "Deprecate a version of a package")
    .with_params(&["registry", "otp", "dry-run"])
    .with_usage(&["<package-spec> <message>"]);

pub struct Deprecate {
    undeprecate: bool,
}

impl Deprecate {
    pub fn new() -> Self {
        Self { undeprecate: false }
    }

    pub fn undeprecate() -> Self {
        Self { undeprecate: true }
    }
}

impl Default for Deprecate {
    fn default() -> Self {
        Self::new()
    }
}

fn meta(undeprecate: bool) -> &'static CommandMeta {
    if undeprecate {
        &undeprecate_meta::META
    } else {
        &META
    }
}

pub mod undeprecate_meta {
    use crate::commands::command::CommandMeta;

    pub static META: CommandMeta =
        CommandMeta::new("undeprecate", "Undeprecate a version of a package")
            .with_params(&["registry", "otp", "dry-run"])
            .with_usage(&["<package-spec>"]);
}

impl Command for Deprecate {
    fn meta(&self) -> &'static CommandMeta {
        meta(self.undeprecate)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let pkg = args.first().filter(|p| !p.is_empty());
        let Some(pkg) = pkg else {
            return Err(meta(self.undeprecate).usage_error(""));
        };
        // `''` is a valid message: it means undeprecate.
        let msg = if self.undeprecate {
            Some(String::new())
        } else {
            args.get(1).cloned()
        };
        let Some(_msg) = msg else {
            return Err(meta(self.undeprecate).usage_error(""));
        };

        let (name, spec) = match pkg.rfind('@') {
            Some(0) | None => (pkg.clone(), "*".to_string()),
            Some(idx) => (pkg[..idx].to_string(), pkg[idx + 1..].to_string()),
        };
        let spec = if spec == "*" || spec.is_empty() {
            "*".to_string()
        } else {
            spec
        };
        if spec != "*" && !spec_looks_valid(&spec) {
            return Err(NpmError::new(format!("invalid version range: {spec}")));
        }

        let _dry_run = npm.store().get_bool("dry-run");
        Err(NpmError::new(format!(
            "Cannot deprecate {name}@{spec}: talking to the registry is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}

/// A permissive range check: `*`, a partial version, or a comparator list.
fn spec_looks_valid(spec: &str) -> bool {
    spec.split("||").all(|part| {
        part.split_whitespace().all(|token| {
            let token = token
                .trim_start_matches(['>', '<', '=', '~', '^'])
                .trim_start_matches('=');
            token.is_empty()
                || token.split('.').next().is_some_and(|major| {
                    major
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit() || c == 'x' || c == '*')
                })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::spec_looks_valid;

    #[test]
    fn accepts_ranges_and_partial_versions() {
        assert!(spec_looks_valid("*"));
        assert!(spec_looks_valid("^1.2.3"));
        assert!(spec_looks_valid(">=1 <2"));
        assert!(!spec_looks_valid("latest"));
    }
}
