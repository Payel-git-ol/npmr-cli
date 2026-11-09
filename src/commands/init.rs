//! Port of `lib/commands/init.js`. Without arguments it scaffolds a
//! `package.json`; with arguments it delegates to `npm exec create-<pkg>`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;

pub static META: CommandMeta = CommandMeta::new("init", "Create a package.json file")
    .with_params(&[
        "init-author-name",
        "init-author-url",
        "init-license",
        "init-module",
        "init-type",
        "init-version",
        "init-private",
        "yes",
        "force",
        "scope",
        "workspace",
        "workspaces",
        "workspaces-update",
        "include-workspace-root",
    ])
    .with_usage(&[
        "<package-spec> (same as `npx create-<package-spec>`)",
        "<@scope> (same as `npx <@scope>/create`)",
    ])
    .workspaces();

pub struct Init;

impl Init {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Init {
    fn default() -> Self {
        Self::new()
    }
}

/// Turns `create-foo`, `@scope` or `foo` into the package that gets executed.
fn initer_package_name(name: &str) -> Option<String> {
    if let Some(scope_only) = name.strip_prefix('@').filter(|s| !s.contains('/')) {
        let (scope, version) = match scope_only.split_once('@') {
            Some((scope, version)) => (scope, Some(version)),
            None => (scope_only, None),
        };
        return Some(match version {
            Some(v) => format!("@{scope}/create@{v}"),
            None => format!("@{scope}/create"),
        });
    }
    let (prefix, rest) = match name.strip_prefix('@') {
        Some(scoped) => match scoped.split_once('/') {
            Some((scope, rest)) => (format!("@{scope}/"), rest),
            None => (String::new(), name),
        },
        None => (String::new(), name),
    };
    let (pkg, version) = match rest.split_once('@') {
        Some((pkg, version)) => (pkg, Some(version)),
        None => (rest, None),
    };
    let name = format!("{prefix}create-{pkg}");
    Some(match version {
        Some(v) => format!("{name}@{v}"),
        None => name,
    })
}

fn guess_name(prefix: &Path) -> String {
    let dir = std::env::current_dir().unwrap_or_else(|_| prefix.to_path_buf());
    let name = if dir == prefix {
        dir.file_name()
    } else {
        prefix.file_name().or_else(|| dir.file_name())
    };
    let raw = name
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sanitized: String = raw
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_start_matches(['-', '.', '_']).to_string();
    if trimmed.is_empty() {
        "app".into()
    } else {
        trimmed
    }
}

fn prompt(label: &str, default: &str) -> String {
    let value =
        crate::utils::read_user_info::read(&format!("{label}: ({default}) ")).unwrap_or_default();
    if value.trim().is_empty() {
        default.to_string()
    } else {
        value
    }
}

fn prompt_optional(label: &str, default: &str) -> Option<String> {
    let value =
        crate::utils::read_user_info::read(&format!("{label}: ({default}) ")).unwrap_or_default();
    let value = value.trim();
    Some(if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    })
    .filter(|v| !v.is_empty())
}

impl Command for Init {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.is_empty() {
            return self.template(npm);
        }
        let Some(initer) = args.first() else {
            return Ok(());
        };
        let Some(package) = initer_package_name(initer) else {
            return Err(NpmError::new(format!(
                "Unrecognized initializer: {initer}\nFor more package binary executing power check out `npx`:\nhttps://docs.npmjs.com/cli/commands/npx"
            ))
            .with_code("EUNSUPPORTED"));
        };
        let mut exec_args = vec![package];
        exec_args.extend(args[1..].iter().cloned());
        npm.exec("exec", &exec_args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let include_root = npm.store().get_bool("include-workspace-root");
        if include_root {
            self.exec(npm, args)?;
        }
        let prefix = npm.local_prefix().to_path_buf();
        if !prefix.join("package.json").is_file() {
            warn(
                "init",
                &[&"Missing package.json. Try with `--include-workspace-root`."],
            );
        }

        let filters = npm.store().get_list("workspace");
        let mut initialized = Vec::new();
        for filter in &filters {
            let path: PathBuf = prefix.join(filter);
            std::fs::create_dir_all(&path)?;
            if args.is_empty() {
                self.template_at(npm, &path)?;
            } else {
                self.exec(npm, args)?;
            }
            initialized.push(path);
        }
        crate::utils::update_workspaces::update_workspaces(
            npm,
            &crate::utils::update_workspaces::UpdateWorkspacesInput {
                local_prefix: prefix,
                workspaces: initialized
                    .iter()
                    .filter_map(|p| {
                        std::fs::read_to_string(p.join("package.json"))
                            .ok()
                            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                            .and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string))
                    })
                    .collect(),
            },
        )
    }
}

impl Init {
    fn template(&mut self, npm: &mut (dyn Npm + '_)) -> Result<()> {
        let cwd = std::env::current_dir().map_err(NpmError::from)?;
        self.template_at(npm, &cwd)
    }

    fn template_at(&mut self, npm: &mut (dyn Npm + '_), path: &Path) -> Result<()> {
        let yes = npm.store().get_bool("yes") || npm.store().get_bool("force");
        if !yes {
            crate::output::set_meta(false);
            crate::output::standard(
                [
                    "This utility will walk you through creating a package.json file.",
                    "It only covers the most common items, and tries to guess sensible defaults.",
                    "",
                    "See `npm help init` for definitive documentation on these fields and exactly what they do.",
                    "",
                    "Use `npm install <pkg>` afterwards to install a package and save it as a dependency in the package.json file.",
                    "",
                    "Press ^C at any time to quit.",
                ]
                .join("\n"),
            );
        }

        let existing = std::fs::read_to_string(path.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let default_name = existing
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| guess_name(npm.prefix().as_path()));
        let default_version = npm.store().get_str("init-version").into_owned();
        let default_license = npm.store().get_str("init-license").into_owned();
        let default_type = npm.store().get_str("init-type").into_owned();
        let default_author = npm.store().get_str("init-author-name").into_owned();
        let default_main = "index.js".to_string();

        let (name, version, description, main, kind, author, license, private) = if yes {
            (
                default_name,
                default_version,
                String::new(),
                default_main,
                default_type,
                default_author,
                default_license,
                npm.store().get_bool("init-private"),
            )
        } else {
            let name = prompt("package name", &default_name);
            let version = prompt("version", &default_version);
            let description = prompt_optional("description", "").unwrap_or_default();
            let main = prompt("entry point", &default_main);
            let kind = prompt("type", &default_type);
            let author = prompt("author", &default_author);
            let license = prompt("license", &default_license);
            let private_answer = crate::utils::read_user_info::read("private: (false) ")
                .unwrap_or_default()
                .to_lowercase();
            (
                name,
                version,
                description,
                main,
                kind,
                author,
                license,
                private_answer == "true" || private_answer == "y" || private_answer == "yes",
            )
        };

        let mut data = Map::new();
        data.insert("name".into(), Value::String(name));
        data.insert("version".into(), Value::String(version));
        if !description.is_empty() {
            data.insert("description".into(), Value::String(description));
        }
        data.insert("main".into(), Value::String(main));
        if !kind.is_empty() {
            data.insert("type".into(), Value::String(kind));
        }
        data.insert(
            "scripts".into(),
            json!({ "test": "echo \"Error: no test specified\" && exit 1" }),
        );
        if !author.is_empty() {
            data.insert("author".into(), Value::String(author));
        }
        if !license.is_empty() {
            data.insert("license".into(), Value::String(license));
        }
        if private {
            data.insert("private".into(), Value::Bool(true));
        }

        let body = serde_json::to_string_pretty(&Value::Object(data)).unwrap_or_default();
        if !yes {
            crate::output::set_meta(false);
            crate::output::standard(format!(
                "About to write to {}\n\n{body}",
                path.join("package.json").display()
            ));
        }
        std::fs::write(path.join("package.json"), format!("{body}\n"))?;
        Ok(())
    }
}
