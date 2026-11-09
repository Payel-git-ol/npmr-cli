//! Port of `lib/commands/stage/publish.js`: `npm publish` with `stage = true`.

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new(
    "publish",
    "Stage a package for publishing, deferring proof-of-presence (2FA) to a later point in time",
)
.with_usage(&["[<tarball>|<folder>]"])
.with_params(crate::commands::publish::PUBLISH_PARAMS)
.workspaces();

pub struct StagePublish;

impl StagePublish {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StagePublish {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let mut publish = crate::commands::publish::Publish::new();
        publish.set_stage(true);
        publish.exec(npm, args)
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}
