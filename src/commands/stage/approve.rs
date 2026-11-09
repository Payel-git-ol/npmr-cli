//! Port of `lib/commands/stage/approve.js`.

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::auth::otplease;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::notice;
use crate::utils::validate_uuid::validate_uuid;

pub static META: CommandMeta = CommandMeta::new(
    "approve",
    "Approve a staged package, publishing it to the npm registry",
)
.with_usage(&["<stage-id>"])
.with_params(&["otp", "registry"])
.with_positionals(1);

pub struct StageApprove;

impl Default for StageApprove {
    fn default() -> Self {
        Self::new()
    }
}

impl StageApprove {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StageApprove {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(stage_id) = args.first() else {
            return Err(self.meta().usage_error("Missing required <stage-id>"));
        };
        validate_uuid(stage_id, "stage-id")?;
        let opts: Vec<(String, serde_json::Value)> = npm.flat_options().into_iter().collect();

        notice("", &[&format!("Approving staged package {stage_id}")]);

        otplease(npm, &opts, |_o| {
            Err(NpmError::new(format!(
                "Could not approve staged package {stage_id}: registry request is not implemented yet"
            ))
            .with_code("ENOTSUP"))
        })?;

        crate::output::set_meta(true);
        crate::output::standard(format!(
            "Staged package {stage_id} approved and published successfully."
        ));
        Ok(())
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}
