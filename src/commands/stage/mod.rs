//! Port of `lib/commands/stage/index.js` plus its subcommands.

pub mod approve;
pub mod download;
pub mod list;
pub mod publish;
pub mod reject;
pub mod view;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new(
    "stage",
    "Stage packages for publishing, deferring proof-of-presence (2FA) to a later point in time",
)
.with_usage(&[]);

/// Subcommand names, in help order.
pub const SUBCOMMANDS: &[&str] = &["publish", "list", "view", "approve", "reject", "download"];

pub struct Stage;

impl Stage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Stage {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(sub) = args.first() else {
            return Err(self.meta().usage_error(""));
        };
        let rest = &args[1..];
        match sub.as_str() {
            "publish" => publish::StagePublish::new().exec(npm, rest),
            "list" => list::StageList::new().exec(npm, rest),
            "view" => view::StageView::new().exec(npm, rest),
            "approve" => approve::StageApprove::new().exec(npm, rest),
            "reject" => reject::StageReject::new().exec(npm, rest),
            "download" => download::StageDownload::new().exec(npm, rest),
            other => Err(crate::utils::errors::NpmError::new(format!(
                "Unknown subcommand: {other}"
            ))
            .with_code("EUNKNOWNCOMMAND")),
        }
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        parse_flags(self.meta(), args)
    }
}

fn parse_flags(meta: &CommandMeta, args: &[String]) -> ParsedFlags {
    crate::commands::command::parse_flags(meta, args)
}
