//! Port of `lib/commands/stage/download.js`.

use std::path::PathBuf;

use crate::commands::command::{Command, CommandMeta, ParsedFlags};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::notice;
use crate::utils::tar::{get_contents, log_tar, read_manifest};
use crate::utils::validate_uuid::validate_uuid;

pub static META: CommandMeta =
    CommandMeta::new("download", "Download the tarball of a staged package for inspection")
        .with_usage(&["<stage-id>"])
        .with_params(&["json", "registry"])
        .with_positionals(1);

pub struct StageDownload;

impl Default for StageDownload {
    fn default() -> Self {
        Self::new()
    }
}

impl StageDownload {
    pub fn new() -> Self {
        Self
    }
}

impl Command for StageDownload {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(stage_id) = args.first() else {
            return Err(self.meta().usage_error("Missing required <stage-id>"));
        };
        validate_uuid(stage_id, "stage-id")?;
        let unicode = npm.store().get_bool("unicode");
        let json = npm.json();

        notice("", &[&format!("Downloading staged package {stage_id}")]);

        let data = fetch_tarball(npm, stage_id)?;

        let manifest = read_manifest(&data)?;
        let contents = get_contents(&manifest, &data)?;
        log_tar(&contents, unicode, json, &contents.name);

        let safe_name = contents.name.replace('@', "").replace('/', "-");
        let filename = format!("{safe_name}-{}-{stage_id}.tgz", contents.version);
        let dest = std::env::current_dir()
            .map_err(NpmError::from)
            .map(|cwd| cwd.join(&filename))?;
        write_file(&dest, &data)?;

        if !json {
            crate::output::set_meta(true);
            crate::output::standard(filename);
        }
        Ok(())
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        crate::commands::command::parse_flags(&META, args)
    }
}

fn write_file(path: &PathBuf, data: &[u8]) -> Result<()> {
    std::fs::write(path, data).map_err(NpmError::from)
}

/// TODO: `GET /-/stage/<id>/tarball`.
fn fetch_tarball(_npm: &dyn Npm, stage_id: &str) -> Result<Vec<u8>> {
    Err(NpmError::new(format!(
        "Could not download staged package {stage_id}: registry request is not implemented yet"
    ))
    .with_code("ENOTSUP"))
}
