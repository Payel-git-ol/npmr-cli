//! Port of `lib/commands/trust/list.js`.

use serde_json::{Map, Value};

use crate::commands::command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags};
use crate::commands::parse_flags;
use crate::commands::trust::circleci::CircleCiProvider;
use crate::commands::trust::command::{
    TrustProvider, dialogue, display_response_body, escaped_name, optional_package_json, trust_uri,
};
use crate::commands::trust::github::GitHubProvider;
use crate::commands::trust::gitlab::GitLabProvider;
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

static META_DEFINITIONS: &[FlagDef] = &[FlagDef::new(
    "id",
    FlagType::String,
    "Trust configuration id",
)];

pub static META: CommandMeta = CommandMeta::new("list", "List trusted relationships for a package")
    .with_usage(&["[package]"])
    .with_params(&["json", "registry"])
    .with_definitions(META_DEFINITIONS)
    .with_positionals(1);

pub struct TrustList;

impl TrustList {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrustList {
    fn default() -> Self {
        Self::new()
    }
}

/// Picks the provider that knows how to read `body.type`.
struct AnyProvider;

impl TrustProvider for AnyProvider {
    fn options_to_body(&self, _values: &Map<String, Value>) -> Value {
        Value::Null
    }

    fn body_to_options(&self, body: &Value) -> Map<String, Value> {
        match body.get("type").and_then(Value::as_str) {
            Some("circleci") => CircleCiProvider.body_to_options(body),
            Some("github") => GitHubProvider.body_to_options(body),
            Some("gitlab") => GitLabProvider.body_to_options(body),
            _ => {
                let mut out = Map::new();
                for key in ["id", "type"] {
                    if let Some(value) = body.get(key) {
                        out.insert(key.into(), value.clone());
                    }
                }
                out
            }
        }
    }
}

impl Command for TrustList {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        parse_flags(&META, args)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let parsed = self.parse_flags(args);
        let content = optional_package_json(npm);
        let package = parsed
            .positionals
            .first()
            .cloned()
            .or_else(|| content.get("name").and_then(Value::as_str).map(str::to_string))
            .ok_or_else(|| {
                NpmError::new(
                    "Package name must be specified either as an argument or in the package.json file",
                )
            })?;

        let uri = trust_uri(&escaped_name(&package));
        let _ = uri;
        // TODO: `GET /-/package/<name>/trust` through `otplease`.
        let body = fetch_trust_configs(npm, &package)?;
        display_response_body(npm, &body, &package, &AnyProvider);
        let _ = dialogue;
        Ok(())
    }
}

/// TODO: registry request.
fn fetch_trust_configs(_npm: &dyn Npm, package: &str) -> Result<Value> {
    Err(NpmError::new(format!(
        "Could not list trust configurations for {package}: registry request is not implemented yet"
    ))
    .with_code("ENOTSUP"))
}
