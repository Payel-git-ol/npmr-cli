//! Port of `lib/commands/trust/circleci.js`.

use serde_json::{Map, Value, json};

use crate::commands::command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags};
use crate::commands::parse_flags;
use crate::commands::trust::command::{
    TrustProvider, TrustValues, create_config, frontend_url, optional_package_json,
};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::validate_uuid::validate_uuid;

const PROVIDER_NAME: &str = "CircleCI";
const PROVIDER_ENTITY: &str = "CircleCI pipeline";

const CLAIM_ORG_ID: &str = "oidc.circleci.com/org-id";
const CLAIM_PROJECT_ID: &str = "oidc.circleci.com/project-id";
const CLAIM_PIPELINE_DEFINITION_ID: &str = "oidc.circleci.com/pipeline-definition-id";
const CLAIM_VCS_ORIGIN: &str = "oidc.circleci.com/vcs-origin";
const CLAIM_CONTEXT_IDS: &str = "oidc.circleci.com/context-ids";

static META_DEFINITIONS: &[FlagDef] = &[
    FlagDef::new("org-id", FlagType::String, "CircleCI organization UUID").required(),
    FlagDef::new("project-id", FlagType::String, "CircleCI project UUID").required(),
    FlagDef::new(
        "pipeline-definition-id",
        FlagType::String,
        "CircleCI pipeline definition UUID",
    )
    .required(),
    FlagDef::new(
        "vcs-origin",
        FlagType::String,
        "CircleCI repository origin in format 'provider/owner/repo'",
    )
    .required(),
    FlagDef::new("context-id", FlagType::List, "CircleCI context UUID to match"),
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
    "circleci",
    "Create a trusted relationship between a package and CircleCI",
)
.with_usage(&["[package] --org-id <uuid> --project-id <uuid> --pipeline-definition-id <uuid> --vcs-origin <origin> [--context-id <uuid>...] [--allow-publish] [--allow-stage-publish] [-y|--yes]"])
.with_params(&["dry-run", "json", "registry", "yes"])
.with_definitions(META_DEFINITIONS).with_positionals(1);

pub struct TrustCircleCi;

impl TrustCircleCi {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrustCircleCi {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CircleCiProvider;

impl TrustProvider for CircleCiProvider {
    fn options_to_body(&self, values: &Map<String, Value>) -> Value {
        let mut claims = Map::new();
        let pairs = [
            (CLAIM_ORG_ID, "orgId"),
            (CLAIM_PROJECT_ID, "projectId"),
            (CLAIM_PIPELINE_DEFINITION_ID, "pipelineDefinitionId"),
            (CLAIM_VCS_ORIGIN, "vcsOrigin"),
        ];
        for (claim, key) in pairs {
            if let Some(value) = values.get(key) {
                claims.insert(claim.into(), value.clone());
            }
        }
        if let Some(Value::Array(context_ids)) = values.get("contextIds") {
            if !context_ids.is_empty() {
                claims.insert(CLAIM_CONTEXT_IDS.into(), Value::Array(context_ids.clone()));
            }
        }
        json!({ "type": "circleci", "claims": claims })
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
        let pairs = [
            (CLAIM_ORG_ID, "orgId"),
            (CLAIM_PROJECT_ID, "projectId"),
            (CLAIM_PIPELINE_DEFINITION_ID, "pipelineDefinitionId"),
            (CLAIM_VCS_ORIGIN, "vcsOrigin"),
            (CLAIM_CONTEXT_IDS, "contextIds"),
        ];
        for (claim, key) in pairs {
            if let Some(value) = claims.and_then(|c| c.get(claim)) {
                out.insert(key.into(), value.clone());
            }
        }
        out
    }
}

fn validate_vcs_origin(value: &str) -> Result<()> {
    if value.contains("://") {
        return Err(NpmError::new(
            "vcs-origin must not include a scheme (e.g., use 'github.com/owner/repo' not 'https://github.com/owner/repo')",
        ));
    }
    if value.split('/').count() < 3 {
        return Err(NpmError::new(
            "vcs-origin must be in format 'provider/owner/repo'",
        ));
    }
    Ok(())
}

fn required_flag<'a>(flags: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    flags
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| NpmError::new(format!("{key} is required")))
}

fn flags_to_options(npm: &mut (dyn Npm + '_), positional: &[String], flags: &Map<String, Value>) -> Result<TrustValues> {
    let content = optional_package_json(npm);
    let pkg_name = positional
        .first()
        .map(String::as_str)
        .or_else(|| content.get("name").and_then(Value::as_str))
        .ok_or_else(|| {
            NpmError::new("Package name must be specified either as an argument or in package.json file")
        })?;

    let org_id = required_flag(flags, "org-id")?;
    let project_id = required_flag(flags, "project-id")?;
    let pipeline_definition_id = required_flag(flags, "pipeline-definition-id")?;
    let vcs_origin = required_flag(flags, "vcs-origin")?;

    validate_uuid(org_id, "org-id")?;
    validate_uuid(project_id, "project-id")?;
    validate_uuid(pipeline_definition_id, "pipeline-definition-id")?;
    validate_vcs_origin(vcs_origin)?;

    let context_ids: Vec<Value> = match flags.get("context-id") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let id = item.as_str().unwrap_or_default();
                validate_uuid(id, "context-id")?;
                Ok(Value::String(id.to_string()))
            })
            .collect::<Result<Vec<_>>>()?,
        Some(Value::String(id)) => {
            validate_uuid(id, "context-id")?;
            vec![Value::String(id.clone())]
        }
        _ => Vec::new(),
    };

    let mut values = Map::new();
    values.insert("package".into(), Value::String(pkg_name.to_string()));
    values.insert("orgId".into(), Value::String(org_id.into()));
    values.insert("projectId".into(), Value::String(project_id.into()));
    values.insert(
        "pipelineDefinitionId".into(),
        Value::String(pipeline_definition_id.into()),
    );
    values.insert("vcsOrigin".into(), Value::String(vcs_origin.into()));
    if !context_ids.is_empty() {
        values.insert("contextIds".into(), Value::Array(context_ids));
    }

    let mut urls = Map::new();
    urls.insert(
        "package".into(),
        frontend_url(npm, pkg_name).map(Value::String).unwrap_or(Value::Null),
    );
    urls.insert(
        "vcsOrigin".into(),
        Value::String(format!("https://{vcs_origin}")),
    );

    Ok(TrustValues {
        values,
        warnings: Vec::new(),
        from_package_json: Map::new(),
        urls,
        permissions: Vec::new(),
    })
}

impl Command for TrustCircleCi {
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
            &CircleCiProvider,
            &parsed.positionals,
            &parsed.flags,
            values,
            PROVIDER_NAME,
            PROVIDER_ENTITY,
        )
    }
}
