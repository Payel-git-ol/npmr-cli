//! Port of `lib/commands/stage/view.js`.

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::key_values::log_stage_item;
use crate::utils::validate_uuid::validate_uuid;

pub static META: CommandMeta = CommandMeta::new("view", "View details of a specific staged package")
    .with_usage(&["<stage-id>"])
    .with_params(&["json", "registry"])
    .with_positionals(1);

pub struct StageView;

impl StageView {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StageView {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(stage_id) = args.first() else {
            return Err(self.meta().usage_error("Missing required <stage-id>"));
        };
        validate_uuid(stage_id, "stage-id")?;
        let json = npm.json();

        let item = fetch_stage_item(npm, stage_id)?;

        if json {
            crate::output::set_meta(true);
            crate::output::standard(serde_json::to_string_pretty(&item).unwrap_or_default());
            return Ok(());
        }
        crate::output::set_meta(true);
        crate::output::standard(log_stage_item(&item));
        Ok(())
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}

/// Whole stage item for `stage-id`. TODO: `GET /-/stage/<id>`.
pub fn fetch_stage_item(_npm: &dyn Npm, stage_id: &str) -> Result<Value> {
    Err(NpmError::new(format!(
        "Could not fetch staged package {stage_id}: registry request is not implemented yet"
    ))
    .with_code("ENOTSUP"))
}
