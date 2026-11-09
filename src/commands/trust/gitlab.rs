//! Port of `lib/commands/trust/gitlab.js`.

use serde_json::{Map, Value, json};

use crate::commands::command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags};
use crate::commands::parse_flags;
use crate::commands::trust::command::{
    TrustProvider, TrustValues, create_config, frontend_url, optional_package_json,
    repository_from_package_json,
};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

const PROVIDER_NAME: &str = "GitLab CI/CD";
const PROVIDER_ENTITY: &str = "GitLab project";
const PROVIDER_FILE: &str = "GitLab CI/CD Pipeline";
const PROVIDER_HOSTNAME: &str = "https://gitlab.com";
const ENTITY_KEY: &str = "project";

static META_DEFINITIONS: &[FlagDef] = &[
    FlagDef::new("file", FlagType::String, "Name of pipeline file (e.g., .gitlab-ci.yml)").required(),
    FlagDef::new(
        "project",
        FlagType::String,
        "Name of the project in the format group/project or group/subgroup/project",
    )
    .with_aliases(&["repo", "repository"]),
    FlagDef::new("environment", FlagType::String, "CI environment name")
        .with_short('e')
        .with_aliases(&["env"]),
    FlagDef::new(
        "allow-publish",
        FlagType::Boolean,
        "Allow npm publish for this trusted publisher configuration",
    ),
    FlagDef::new(
        "allow-stage-publish",
        FlagType::Boolean,
        "Allow npm stage publish for this trusted publisher configuration",
    )
    .with_aliases(&["allow-staged-publish"]),
];

pub static META: CommandMeta = CommandMeta::new(
    "gitlab",
    "Create a trusted relationship between a package and GitLab CI/CD",
)
.with_usage(&["[package] --file [--project|--repo|--repository] [--env|--environment] [--allow-publish] [--allow-stage-publish] [-y|--yes]"])
.with_params(&["dry-run", "json", "registry", "yes"])
.with_definitions(META_DEFINITIONS).with_positionals(1);

pub struct TrustGitLab;

impl TrustGitLab {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrustGitLab {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GitLabProvider;

impl TrustProvider for GitLabProvider {
    fn options_to_body(&self, values: &Map<String, Value>) -> Value {
        let mut config_ref = Map::new();
        if let Some(file) = values.get("file") {
            config_ref.insert("file".into(), file.clone());
        }
        let mut claims = Map::new();
        if let Some(project) = values.get(ENTITY_KEY) {
            claims.insert("project_path".into(), project.clone());
        }
        claims.insert("ci_config_ref_uri".into(), Value::Object(config_ref));
        if let Some(env) = values.get("environment") {
            claims.insert("environment".into(), env.clone());
        }
        json!({ "type": "gitlab", "claims": claims })
    }

    fn body_to_options(&self, body: &Value) -> Map<String, Value> {
        let mut out = Map::new();
        if let Some(id) = body.get("id") {
            out.insert("id".into(), id.clone());
        }
        if let Some(kind) = body.get("type") {
            out.insert("type".into(), kind.clone());
        }
        let claims = body.get("claims");
        if let Some(file) = claims
            .and_then(|c| c.get("ci_config_ref_uri"))
            .and_then(|w| w.get("file"))
        {
            out.insert("file".into(), file.clone());
        }
        if let Some(project) = claims.and_then(|c| c.get("project_path")) {
            out.insert(ENTITY_KEY.into(), project.clone());
        }
        if let Some(env) = claims.and_then(|c| c.get("environment")) {
            out.insert("environment".into(), env.clone());
        }
        out
    }
}

fn entity_url(entity: &str, file: Option<&str>) -> String {
    match file {
        Some(file) => format!("{PROVIDER_HOSTNAME}/{entity}/-/blob/HEAD/{file}"),
        None => format!("{PROVIDER_HOSTNAME}/{entity}"),
    }
}

fn validate_entity(entity: &str) -> Result<()> {
    if entity.split('/').count() < 2 {
        return Err(NpmError::new(format!(
            "{PROVIDER_ENTITY} must be specified in the format group/project or group/subgroup/project"
        )));
    }
    Ok(())
}

fn validate_file(file: &str) -> Result<()> {
    let is_base_name = std::path::Path::new(file)
        .file_name()
        .is_some_and(|name| name == file);
    if !is_base_name {
        return Err(NpmError::new(
            "GitLab CI/CD pipeline file must be just a file not a path",
        ));
    }
    Ok(())
}

fn flags_to_options(
    npm: &mut (dyn Npm + '_),
    positional: &[String],
    flags: &Map<String, Value>,
) -> Result<TrustValues> {
    let content = optional_package_json(npm);
    let pkg_positional = positional.first().map(String::as_str);
    let pkg_json_name = content.get("name").and_then(Value::as_str);
    let git = repository_from_package_json(&content);
    let match_pkg = pkg_positional.is_none() || pkg_positional == pkg_json_name;
    let pkg_name = pkg_positional.or(pkg_json_name);
    let used_pkg_name_from_json = pkg_positional.is_none() && pkg_json_name.is_some();

    let git_matches_provider = git
        .as_ref()
        .is_some_and(|(host, _)| host == PROVIDER_HOSTNAME.trim_start_matches("https://"));
    let invalid_pkg_json_provider_type = match_pkg && git.is_some() && !git_matches_provider;

    let mut warnings = Vec::new();
    let Some(pkg_name) = pkg_name else {
        return Err(NpmError::new(
            "Package name must be specified either as an argument or in package.json file",
        ));
    };

    let entity_from_flag = flags.get(ENTITY_KEY).and_then(Value::as_str);
    let entity_from_json = git
        .as_ref()
        .map(|(_, path)| path.as_str())
        .filter(|_| !invalid_pkg_json_provider_type);
    let entity = entity_from_flag.or(entity_from_json);
    let used_repository_in_json = entity_from_flag.is_none() && entity_from_json.is_some();
    let mismatch_json_repository =
        match_pkg && git.is_some() && entity_from_json.is_some_and(|j| Some(j) != entity);

    if invalid_pkg_json_provider_type {
        let message = format!("Repository in package.json is not a {PROVIDER_ENTITY}");
        if entity_from_flag.is_none() {
            return Err(NpmError::new(message));
        }
        warnings.push(message);
    } else if mismatch_json_repository {
        if let Some((_, json_repo)) = &git {
            warnings.push(format!(
                "Repository in package.json ({json_repo}) differs from provided {PROVIDER_ENTITY} ({})",
                entity.unwrap_or_default()
            ));
        }
    }

    let file = flags.get("file").and_then(Value::as_str).ok_or_else(|| {
        NpmError::new(format!("{PROVIDER_FILE} must be specified with the file option"))
    })?;
    if !file.ends_with(".yml") && !file.ends_with(".yaml") {
        return Err(NpmError::new(format!(
            "{PROVIDER_FILE} must end in .yml or .yaml"
        )));
    }
    validate_file(file)?;

    let Some(entity) = entity else {
        let hint = if match_pkg {
            format!(
                "{PROVIDER_ENTITY} must be specified with {ENTITY_KEY} option or inferred from the package.json repository field"
            )
        } else {
            format!("{PROVIDER_ENTITY} must be specified with {ENTITY_KEY} option")
        };
        return Err(NpmError::new(hint));
    };
    validate_entity(entity)?;

    let mut values = Map::new();
    values.insert("package".into(), Value::String(pkg_name.to_string()));
    values.insert("file".into(), Value::String(file.to_string()));
    values.insert(ENTITY_KEY.into(), Value::String(entity.to_string()));
    if let Some(env) = flags.get("environment").and_then(Value::as_str) {
        values.insert("environment".into(), Value::String(env.to_string()));
    }

    let mut from_package_json = Map::new();
    from_package_json.insert(ENTITY_KEY.into(), Value::Bool(used_repository_in_json));
    from_package_json.insert("package".into(), Value::Bool(used_pkg_name_from_json));

    let mut urls = Map::new();
    urls.insert(
        "package".into(),
        frontend_url(npm, pkg_name).map(Value::String).unwrap_or(Value::Null),
    );
    urls.insert(ENTITY_KEY.into(), Value::String(entity_url(entity, None)));
    urls.insert("file".into(), Value::String(entity_url(entity, Some(file))));

    Ok(TrustValues {
        values,
        warnings,
        from_package_json,
        urls,
        permissions: Vec::new(),
    })
}

impl Command for TrustGitLab {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        parse_flags(&META, args)
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let parsed = self.parse_flags(args);
        let values = flags_to_options(npm, &parsed.positionals, &parsed.flags)?;
        create_config(
            npm,
            &META,
            &GitLabProvider,
            &parsed.positionals,
            &parsed.flags,
            values,
            PROVIDER_NAME,
            PROVIDER_ENTITY,
        )
    }
}
