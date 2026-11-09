//! Port of `lib/commands/publish.js`.

use serde_json::Value;

use crate::commands::command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags};
use crate::commands::parse_flags;
use crate::npm::Npm;
use crate::store::ConfigSource;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{notice, warn, verbose};
use crate::utils::tar::{get_contents, log_tar};
use crate::utils::set_workspaces::set_workspaces;

pub static PUBLISH_PARAMS: &[&str] = &[
    "tag",
    "access",
    "dry-run",
    "otp",
    "workspace",
    "workspaces",
    "include-workspace-root",
    "provenance",
];

pub static META: CommandMeta = CommandMeta::new("publish", "Publish a package")
    .with_usage(&["<package-spec>"])
    .with_params(PUBLISH_PARAMS)
    .with_definitions(&[FlagDef::new("stage", FlagType::Boolean, "Stage instead of publishing")])
    .workspaces();

pub struct Publish {
    stage: bool,
}

impl Publish {
    pub fn new() -> Self {
        Self { stage: false }
    }

    pub fn set_stage(&mut self, stage: bool) {
        self.stage = stage;
    }

    fn command_name(&self) -> &'static str {
        if self.stage {
            "stage"
        } else {
            "publish"
        }
    }
}

impl Default for Publish {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Publish {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let args = if args.is_empty() {
            vec![".".to_string()]
        } else {
            args.to_vec()
        };
        if args.len() != 1 {
            return Err(self.meta().usage_error(""));
        }
        publish_one(self.stage, npm, &args[0], None)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let use_workspaces = args.is_empty() || args.iter().any(|a| a == ".");
        if !use_workspaces {
            warn("Ignoring workspaces for specified package(s)", &[&""]);
            return self.exec(npm, args);
        }
        let workspaces = set_workspaces(npm)?;
        for (name, path) in &workspaces {
            if let Err(err) = publish_one(self.stage, npm, path, Some(name)) {
                if err.code.as_deref() != Some("EPRIVATE") {
                    return Err(err);
                }
                warn(
                    self.command_name(),
                    &[
                        &"Skipping workspace ",
                        &name,
                        &", marked as ",
                        &"private",
                    ],
                );
            }
        }
        Ok(())
    }
}

fn publish_one(
    is_stage: bool,
    npm: &mut (dyn Npm + '_),
    spec: &str,
    workspace: Option<&str>,
) -> Result<()> {
    verbose(if is_stage { "stage" } else { "publish" }, &[&spec]);

    let dry_run = npm.store().get_bool("dry-run");
    let json = npm.json();
    let default_tag = npm.store().get_str("tag");
    if semver::VersionReq::parse(&default_tag).is_ok() {
        return Err(NpmError::new(format!(
            "Tag name must not be a valid SemVer range: {}",
            default_tag.trim()
        )));
    }

    let manifest = get_manifest(npm, spec)?;
    let package_private = manifest
        .get("private")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let package_name = manifest
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if !npm.store().get_bool("force") {
        let is_default_tag = npm.store().is_default("tag")
            && manifest.get("publishConfig").and_then(|p| p.get("tag")).is_none();
        let is_prerelease = semver::Version::parse(version)
            .map(|v| !v.pre.is_empty())
            .unwrap_or(false);
        if is_prerelease && is_default_tag {
            return Err(NpmError::new(
                "You must specify a tag using --tag when publishing a prerelease version.",
            ));
        }
    }

    assert_no_package_extensions(&manifest)?;

    let registry = npm.store().get_str("registry");
    let has_creds = credentials_present(npm, &registry);
    if workspace.is_some() && package_private {
        return Err(NpmError::new(format!(
            "This package has been marked as private\n  Remove the 'private' field from the package.json to publish it."
        ))
        .with_code("EPRIVATE"));
    }
    if !has_creds {
        let msg = format!("This command requires you to be logged in to {registry}");
        if dry_run {
            warn(if is_stage { "stage" } else { "publish" }, &[&format!("{msg} (dry-run)")]);
        } else {
            return Err(NpmError::new(msg).with_code("ENEEDAUTH"));
        }
    }

    let access = match npm.store().get("access") {
        None | Some(Value::Null) => "default".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    };
    let verb = if is_stage { "Staging" } else { "Publishing" };
    let mut msg = format!("{verb} to {registry} with tag {default_tag} and {access} access");
    if dry_run {
        msg.push_str(" (dry-run)");
    }
    notice("", &[&msg]);

    // TODO: pack the tarball (libnpmpack) and POST it. Until then the dry run
    // still reports what would have been sent.
    let tarball = pack_tarball(npm, spec, &manifest)?;
    let contents = get_contents(&manifest, &tarball)?;

    if json {
        log_tar(&contents, npm.store().get_bool("unicode"), true, &contents.name);
    } else {
        log_tar(&contents, npm.store().get_bool("unicode"), false, &contents.name);
    }

    if !dry_run {
        return Err(NpmError::new(format!(
            "Could not {verb_lower} {}: registry upload is not implemented yet",
            contents.id,
            verb_lower = if is_stage { "stage" } else { "publish" },
        ))
        .with_code("ENOTSUP"));
    }

    if !json && !npm.silent() {
        if is_stage {
            crate::output::set_meta(true);
            crate::output::standard(format!("+ {} (staged)", contents.id));
            notice(
                if is_stage { "stage" } else { "publish" },
                &[&format!(
                    "package {} has been staged with tag {default_tag}",
                    contents.id
                )],
            );
        } else {
            crate::output::set_meta(false);
            crate::output::standard(format!("+ {}", contents.id));
        }
    }
    let _ = (package_name, workspace);
    Ok(())
}

fn credentials_present(npm: &dyn Npm, registry: &str) -> bool {
    let host = crate::utils::auth::registry_host(registry);
    let read = |key: &str| {
        npm.store()
            .get(&format!("//{host}/:{key}"))
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty())
    };
    read("_authToken")
        || read("_auth")
        || (read("certfile") && read("keyfile"))
        || npm.store().get_str("username") == "npm"
}

/// Reads the manifest for `spec`. Directory specs are read from disk, tarballs
/// are inspected, and registry specs are not fetched yet.
fn get_manifest(npm: &dyn Npm, spec: &str) -> Result<Value> {
    let path = std::path::Path::new(spec);
    if path.is_dir() || spec == "." {
        let manifest_path = path.join("package.json");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|_| NpmError::new(format!("Could not read {}", manifest_path.display())))?;
        return serde_json::from_str(&text)
            .map_err(|_| NpmError::new("Failed to parse package.json").with_code("EJSONPARSE"));
    }
    if path.extension().is_some_and(|e| e == "tgz" || e == "gz") {
        let data = std::fs::read(path)?;
        return crate::utils::tar::read_manifest(&data);
    }
    let _ = npm;
    Err(NpmError::new(format!(
        "Fetching the manifest for {spec} is not implemented yet"
    ))
    .with_code("ENOTSUP"))
}

/// Packs a directory into a gzip-less ustar archive of its files.
fn pack_tarball(_npm: &dyn Npm, spec: &str, _manifest: &Value) -> Result<Vec<u8>> {
    let path = std::path::Path::new(spec);
    if !path.is_dir() {
        return std::fs::read(path).map_err(NpmError::from);
    }

    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name == "node_modules" || name == ".git" {
                continue;
            }
            let full = entry.path();
            if full.is_dir() {
                stack.push(full);
                continue;
            }
            let relative = full.strip_prefix(path).unwrap_or(&full);
            let archive_path = format!("package/{}", relative.to_string_lossy().replace('\\', "/"));
            files.push((archive_path, std::fs::read(&full)?));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(crate::utils::tar_reader::write_tar(&files))
}

fn assert_no_package_extensions(manifest: &Value) -> Result<()> {
    let private = manifest
        .get("private")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !private && manifest.get("packageExtensions").is_some() {
        return Err(NpmError::new(
            "packageExtensions is only honored at the project root and must not be published.",
        )
        .with_code("EPACKAGEEXTENSIONS"));
    }
    Ok(())
}

/// Publishes a prebuilt tarball; shared with the stage subcommand.
pub fn apply_publish_config(npm: &mut (dyn Npm + '_), manifest: &Value) {
    let Some(Value::Object(config)) = manifest.get("publishConfig") else {
        return;
    };
    let cli_keys: Vec<String> = config.keys().cloned().collect();
    for key in cli_keys {
        if let Some(value) = config.get(&key) {
            npm.store_mut().set(value.clone(), &key, ConfigSource::Cli);
        }
    }
}

/// Re-parses `args` with the publish meta; used by `stage publish`.
pub fn parse_publish_flags(args: &[String]) -> ParsedFlags {
    parse_flags(&META, args)
}
