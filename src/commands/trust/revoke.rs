//! Port of `lib/commands/trust/revoke.js`.

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags};
use crate::commands::parse_flags;
use crate::commands::trust::command::{
    confirm_operation, dialogue, optional_package_json, trust_uri_with_id,
};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

static META_DEFINITIONS: &[FlagDef] = &[FlagDef::new(
        "id",
        FlagType::String,
        "ID of the trusted relationship to revoke",
    )
    .required()];

pub static META: CommandMeta = CommandMeta::new("revoke", "Revoke a trusted relationship for a package")
    .with_usage(&["[package] --id=<trust-id>"])
    .with_params(&["dry-run", "registry"])
        .with_definitions(META_DEFINITIONS).with_positionals(1);

pub struct TrustRevoke;

impl TrustRevoke {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrustRevoke {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for TrustRevoke {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        parse_flags(&META, args)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let parsed = self.parse_flags(args);
        let dry_run = npm.store().get_bool("dry-run");
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
        let id = parsed
            .flags
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NpmError::new(
                    "ID of the trusted relationship to revoke must be specified with the --id option",
                )
            })?;

        dialogue(
            npm,
            &format!(
                "Attempting to revoke trusted configuration for package {package} with id {id}"
            ),
        );
        if dry_run {
            return Ok(());
        }
        if !npm.store().get_bool("yes") {
            confirm_operation(None)?;
        }

        let uri = trust_uri_with_id(&package, id);
        // TODO: `DELETE <uri>` through `otplease`.
        let _ = uri;
        Err(NpmError::new(format!(
            "Could not revoke trust configuration {id} for {package}: registry request is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}
