//! Port of `lib/commands/stage/reject.js`.

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::auth::otplease;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{notice, warn};
use crate::utils::validate_uuid::validate_uuid;

pub static META: CommandMeta = CommandMeta::new(
    "reject",
    "Reject a staged package, removing it from the registry",
)
.with_usage(&["<stage-id>"])
.with_params(&["otp", "registry"])
.with_positionals(1);

pub struct StageReject;

impl StageReject {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StageReject {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(stage_id) = args.first() else {
            return Err(self.meta().usage_error("Missing required <stage-id>"));
        };
        validate_uuid(stage_id, "stage-id")?;
        let opts: Vec<(String, Value)> = npm.flat_options().into_iter().collect();

        notice("", &[&format!("Rejecting staged package {stage_id}")]);
        warn(
            "",
            &[&"Rejecting will permanently delete this staged publish record and tarball from the registry."],
        );

        otplease(npm, &opts, |_o| {
            Err(NpmError::new(format!(
                "Could not reject staged package {stage_id}: registry request is not implemented yet"
            ))
            .with_code("ENOTSUP"))
        })?;

        crate::output::set_meta(true);
        crate::output::standard(format!("Staged package {stage_id} has been rejected."));
        Ok(())
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}
