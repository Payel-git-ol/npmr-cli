//! Port of `lib/trust-cmd.js`: shared behaviour for `npm trust <provider>`
//! commands (create, list and revoke trusted publisher configurations).

use serde_json::{Map, Value};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;

pub const NPM_FRONTEND: &str = "https://www.npmjs.com";

pub const CREATE_PACKAGE: &str = "createPackage";
pub const CREATE_STAGED_PACKAGE: &str = "createStagedPackage";

/// Maps registry permission names onto the labels shown to users.
pub fn permission_labels(permission: &str) -> Option<&'static str> {
    match permission {
        CREATE_PACKAGE => Some("publish"),
        CREATE_STAGED_PACKAGE => Some("stage publish"),
        _ => None,
    }
}

pub fn format_permissions(permissions: &[Value]) -> Option<String> {
    if permissions.is_empty() {
        return None;
    }
    Some(
        permissions
            .iter()
            .map(|p| {
                let raw = p.as_str().unwrap_or_default();
                permission_labels(raw).unwrap_or(raw).to_string()
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Options that describe one trust configuration, as printed by
/// [`log_options`] and turned into a request body by each provider.
#[derive(Debug, Clone, Default)]
pub struct TrustValues {
    pub values: Map<String, Value>,
    pub warnings: Vec<String>,
    pub from_package_json: Map<String, Value>,
    pub urls: Map<String, Value>,
    pub permissions: Vec<Value>,
}

/// Provider-specific hooks. Implemented by `github`, `gitlab` and `circleci`.
pub trait TrustProvider {
    fn permission_flags(&self, flags: &Map<String, Value>) -> Result<(bool, bool)> {
        let allow_publish = flags.get("allow-publish").and_then(Value::as_bool).unwrap_or(false);
        let allow_stage = flags
            .get("allow-stage-publish")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !allow_publish && !allow_stage {
            return Err(NpmError::new(
                "At least one permission flag is required (--allow-publish, --allow-stage-publish)",
            ));
        }
        Ok((allow_publish, allow_stage))
    }

    fn options_to_body(&self, values: &Map<String, Value>) -> Value;
    fn body_to_options(&self, body: &Value) -> Map<String, Value>;
}

/// Builds the permission list from the two permission flags.
pub fn permissions_from_flags(allow_publish: bool, allow_stage_publish: bool) -> Vec<Value> {
    let mut permissions = Vec::new();
    if allow_publish {
        permissions.push(Value::String(CREATE_PACKAGE.into()));
    }
    if allow_stage_publish {
        permissions.push(Value::String(CREATE_STAGED_PACKAGE.into()));
    }
    permissions
}

/// The username-style registry API path for a package's trust configs.
pub fn trust_uri(package: &str) -> String {
    format!("/-/package/{}/trust", escaped_name(package))
}

pub fn trust_uri_with_id(package: &str, id: &str) -> String {
    let encoded = percent_encoding::utf8_percent_encode(id, percent_encoding::NON_ALPHANUMERIC)
        .to_string();
    format!("{}/{encoded}", trust_uri(package))
}

/// `@scope/name` -> `@scope%2fname`.
pub fn escaped_name(package: &str) -> String {
    package.replace('/', "%2f")
}

pub fn registry_is_default(npm: &dyn Npm) -> bool {
    let registry = npm.store().get_str("registry");
    registry == crate::store::DEFAULT_REGISTRY
}

pub fn frontend_url(npm: &dyn Npm, package: &str) -> Option<String> {
    registry_is_default(npm).then(|| format!("{NPM_FRONTEND}/package/{package}"))
}

/// Warns when a non-default registry is used because it may not support
/// trusted publishing.
pub fn warn_if_custom_registry(npm: &dyn Npm) {
    if !registry_is_default(npm) {
        warn(
            "trust",
            &[&format!(
                "Registry {} may not support trusted publishing",
                npm.store().get_str("registry")
            )],
        );
    }
}

/// Reads `package.json` from the prefix, returning an empty object when the
/// file is missing or unreadable.
pub fn optional_package_json(npm: &dyn Npm) -> Map<String, Value> {
    let path = npm.prefix().join("package.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Extracts `repository` from `package.json` for `type` (`github`/`gitlab`),
/// returning `owner/repo`.
pub fn repository_from_package_json(
    content: &Map<String, Value>,
) -> Option<(String, String)> {
    let repository = content.get("repository")?;
    let url = match repository {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("url")?.as_str()?.to_string(),
        _ => return None,
    };
    let (host, path) = parse_hosted_url(&url)?;
    Some((host, path))
}

fn parse_hosted_url(url: &str) -> Option<(String, String)> {
    let trimmed = url
        .trim_start_matches("git+")
        .trim_start_matches("git://")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@");
    let (host, rest) = trimmed.split_once('/')?;
    let host = host.split_once(':').map(|(h, _)| h).unwrap_or(host);
    let rest = rest.trim_end_matches(".git").split('#').next().unwrap_or(rest);
    let path = if rest.starts_with("scm/") {
        rest.trim_start_matches("scm/").split_once('/').map(|(_, p)| p)?
    } else {
        rest
    };
    Some((host.to_string(), path.to_string()))
}

/// Prints the values/warnings/URLs block that every trust subcommand shows.
pub fn log_options(npm: &dyn Npm, options: &TrustValues, pad: bool) {
    for warning in &options.warnings {
        warn("trust", &[warning]);
    }

    if npm.json() {
        let mut values = options.values.clone();
        if !options.permissions.is_empty() {
            values.insert(
                "permissions".into(),
                Value::Array(options.permissions.clone()),
            );
        }
        crate::output::set_meta(true);
        crate::output::standard(
            serde_json::to_string_pretty(&Value::Object(values)).unwrap_or_default(),
        );
        return;
    }

    let values = &options.values;
    let mut lines = Vec::new();
    if let Some(kind) = values.get("type").and_then(Value::as_str) {
        lines.push(format!("type: {kind}"));
    }
    if let Some(id) = values.get("id").and_then(Value::as_str) {
        lines.push(format!("id: {id}"));
    }
    for (key, value) in values {
        if key == "type" || key == "id" || value.is_null() {
            continue;
        }
        let rendered = match value {
            Value::String(s) => s.clone(),
            Value::Array(items) => items
                .iter()
                .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        let mut line = format!("{key}: {rendered}");
        if options.from_package_json.contains_key(key) {
            line.push_str(" (from package.json)");
        }
        lines.push(line);
    }
    if let Some(permissions) = format_permissions(&options.permissions) {
        lines.push(format!("permissions: {permissions}"));
    }

    if pad {
        crate::output::standard("");
    }
    crate::output::set_meta(true);
    crate::output::standard(lines.join("\n"));

    let url_lines: Vec<String> = values
        .keys()
        .filter_map(|key| options.urls.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    if !url_lines.is_empty() {
        crate::output::standard("");
        crate::output::standard(url_lines.join("\n"));
    }
    if pad {
        crate::output::standard("");
    }
}

/// Asks the user to confirm a destructive/state-changing operation.
pub fn confirm_operation(yes: Option<bool>) -> Result<()> {
    match yes {
        Some(true) => Ok(()),
        Some(false) => Err(NpmError::new("User cancelled operation")),
        None => {
            if !crate::utils::is_windows::is_windows() && !stdin_is_tty() {
                return Err(NpmError::new("User cancelled operation"));
            }
            let answer = crate::utils::read_user_info::read("Do you want to proceed? (y/N) ")
                .unwrap_or_default();
            let normalized = answer.to_lowercase();
            if normalized == "y" || normalized == "yes" {
                Ok(())
            } else {
                Err(NpmError::new("User cancelled operation"))
            }
        }
    }
}

fn stdin_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Prints the configs returned by a create/list request.
pub fn display_response_body(npm: &dyn Npm, body: &Value, package: &str, provider: &dyn TrustProvider) {
    let items: Vec<&Value> = match body {
        Value::Array(items) => items.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    };
    if items.is_empty() {
        dialogue(
            npm,
            &format!("No trust configurations found for package ({package})"),
        );
        return;
    }
    for config in items {
        let values = provider.body_to_options(config);
        let permissions = config
            .get("permissions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        crate::output::standard("");
        log_options(
            npm,
            &TrustValues {
                values,
                permissions,
                ..Default::default()
            },
            false,
        );
    }
    crate::output::standard("");
}

/// Non-log text that is always shown unless `--json` is set.
pub fn dialogue(npm: &dyn Npm, message: &str) {
    if !npm.json() {
        crate::output::standard(message);
    }
}

/// Shared `create` flow: validate flags, show what will be created, ask for
/// confirmation and POST the config. TODO: perform the request.
#[expect(clippy::too_many_arguments, reason = "mirrors the upstream JS signature")]
pub fn create_config(
    npm: &mut (dyn Npm + '_),
    meta: &CommandMeta,
    provider: &dyn TrustProvider,
    positional_args: &[String],
    flags: &Map<String, Value>,
    mut options: TrustValues,
    provider_name: &str,
    provider_entity: &str,
) -> Result<()> {
    let _ = positional_args;
    let dry_run = npm.store().get_bool("dry-run");
    let yes = npm.store().get("yes").and_then(Value::as_bool);

    let (allow_publish, allow_stage_publish) = provider.permission_flags(flags)?;
    let permissions = permissions_from_flags(allow_publish, allow_stage_publish);

    let package = options
        .values
        .get("package")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    dialogue(
        npm,
        &format!("Establishing trust between {package} package and {provider_name}"),
    );
    dialogue(
        npm,
        &format!("Anyone with {provider_entity} write access can publish to {package}"),
    );
    dialogue(npm, "Two-factor authentication is required for this operation");
    warn_if_custom_registry(npm);
    options.permissions = permissions;
    log_options(npm, &options, true);

    if dry_run {
        return Ok(());
    }
    confirm_operation(yes)?;

    let body = provider.options_to_body(&options.values);
    let _ = body;
    let _ = meta;
    Err(NpmError::new(format!(
        "Could not create trust configuration for {package}: registry request is not implemented yet"
    ))
    .with_code("ENOTSUP"))
}

/// Flags of a trust command as a `Map`, plus its positionals.
pub fn split_flags(command: &dyn Command, args: &[String]) -> (Map<String, Value>, Vec<String>) {
    let parsed = command.parse_flags(args);
    (parsed.flags, parsed.positionals)
}
