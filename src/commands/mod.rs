//! Command registry. One module per file in `lib/commands`, plus the shared
//! bases (`ArboristCmd`, `PackageUrlCommand`, `LifecycleCmd`, `TrustCommand`).
//! The registry maps canonical names onto [`CommandMeta`] so help output and
//! flag parsing can work without instantiating a command.

mod command;

pub mod access;
pub mod approve_scripts;
pub mod audit;
pub mod bugs;
pub mod cache;
pub mod ci;
pub mod completion;
pub mod config;
pub mod dedupe;
pub mod deny_scripts;
pub mod deprecate;
pub mod diff;
pub mod dist_tag;
pub mod docs;
pub mod doctor;
pub mod edit;
pub mod exec;
pub mod explain;
pub mod explore;
pub mod find_dupes;
pub mod fund;
pub mod get;
pub mod help;
pub mod help_search;
pub mod init;
pub mod install;
pub mod install_ci_test;
pub mod install_scripts;
pub mod install_test;
pub mod link;
pub mod login;
pub mod logout;
pub mod ls;
pub mod org;
pub mod outdated;
pub mod owner;
pub mod pack;
pub mod patch;
pub mod ping;
pub mod pkg;
pub mod prefix;
pub mod profile;
pub mod prune;
pub mod publish;
pub mod query;
pub mod rebuild;
pub mod repo;
pub mod restart;
pub mod root;
pub mod run;
pub mod sbom;
pub mod search;
pub mod set;
pub mod stage;
pub mod start;
pub mod stop;
pub mod team;
pub mod test;
pub mod token;
pub mod trust;
pub mod undeprecate;
pub mod uninstall;
pub mod unpublish;
pub mod update;
pub mod version;
pub mod view;
pub mod whoami;

pub use command::{Command, CommandMeta, FlagDef, FlagType, ParsedFlags, parse_flags};

use crate::store::ConfigSource;
use crate::utils::errors::{NpmError, Result};
use serde_json::Value;
use std::sync::OnceLock;

/// Returns the static metadata for a canonical command name.
pub fn meta_for(name: &str) -> Option<&'static CommandMeta> {
    static TABLE: OnceLock<std::collections::BTreeMap<&'static str, CommandMeta>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut map = std::collections::BTreeMap::new();
        macro_rules! register {
            ($($module:ident),* $(,)?) => {
                $( map.insert($module::META.name, $module::META.clone()); )*
            };
        }
        register!(
            access,
            approve_scripts,
            audit,
            bugs,
            cache,
            ci,
            completion,
            config,
            dedupe,
            deny_scripts,
            deprecate,
            diff,
            dist_tag,
            docs,
            doctor,
            edit,
            exec,
            explain,
            explore,
            find_dupes,
            fund,
            get,
            help,
            help_search,
            init,
            install,
            install_ci_test,
            install_scripts,
            install_test,
            link,
            login,
            logout,
            ls,
            org,
            outdated,
            owner,
            pack,
            patch,
            ping,
            pkg,
            prefix,
            profile,
            prune,
            publish,
            query,
            rebuild,
            repo,
            restart,
            root,
            run,
            sbom,
            search,
            set,
            start,
            stop,
            team,
            test,
            token,
            undeprecate,
            uninstall,
            unpublish,
            update,
            version,
            view,
            whoami,
        );
        map
    });
    table.get(name)
}

/// Whether `name` is a real command (after `deref`).
pub fn canonical(name: &str) -> Option<&'static str> {
    meta_for(name).map(|meta| meta.name)
}

pub fn description_for(name: &str) -> Option<String> {
    meta_for(name).map(|meta| meta.description.to_string())
}

pub fn usage_for(name: &str, parent_name: Option<&str>, include_descriptions: bool) -> String {
    meta_for(name)
        .map(|meta| meta.usage_text(parent_name, include_descriptions))
        .unwrap_or_default()
}

/// Instantiates a command by canonical name.
pub fn create(name: &str) -> Result<Box<dyn Command>> {
    Ok(match name {
        "access" => Box::new(access::Access::new()),
        "approve-scripts" => Box::new(approve_scripts::ApproveScripts::new()),
        "audit" => Box::new(audit::Audit::new()),
        "bugs" => Box::new(bugs::Bugs::new()),
        "cache" => Box::new(cache::Cache::new()),
        "ci" => Box::new(ci::Ci::new()),
        "completion" => Box::new(completion::Completion::new()),
        "config" => Box::new(config::Config::new()),
        "dedupe" => Box::new(dedupe::Dedupe::new()),
        "deny-scripts" => Box::new(deny_scripts::DenyScripts::new()),
        "deprecate" => Box::new(deprecate::Deprecate::new()),
        "diff" => Box::new(diff::Diff::new()),
        "dist-tag" => Box::new(dist_tag::DistTag::new()),
        "docs" => Box::new(docs::Docs::new()),
        "doctor" => Box::new(doctor::Doctor::new()),
        "edit" => Box::new(edit::Edit::new()),
        "exec" => Box::new(exec::Exec::new()),
        "explain" => Box::new(explain::Explain::new()),
        "explore" => Box::new(explore::Explore::new()),
        "find-dupes" => Box::new(find_dupes::FindDupes::new()),
        "fund" => Box::new(fund::Fund::new()),
        "get" => Box::new(get::Get::new()),
        "help" => Box::new(help::Help::new()),
        "help-search" => Box::new(help_search::HelpSearch::new()),
        "init" => Box::new(init::Init::new()),
        "install" => Box::new(install::Install::new()),
        "install-ci-test" => Box::new(install_ci_test::InstallCiTest::new()),
        "install-scripts" => Box::new(install_scripts::InstallScripts::new()),
        "install-test" => Box::new(install_test::InstallTest::new()),
        "link" => Box::new(link::Link::new()),
        "ll" => Box::new(ls::Ll::new()),
        "login" => Box::new(login::Login::new()),
        "logout" => Box::new(logout::Logout::new()),
        "ls" => Box::new(ls::Ls::new()),
        "org" => Box::new(org::Org::new()),
        "outdated" => Box::new(outdated::Outdated::new()),
        "owner" => Box::new(owner::Owner::new()),
        "pack" => Box::new(pack::Pack::new()),
        "patch" => Box::new(patch::Patch::new()),
        "ping" => Box::new(ping::Ping::new()),
        "pkg" => Box::new(pkg::Pkg::new()),
        "prefix" => Box::new(prefix::Prefix::new()),
        "profile" => Box::new(profile::Profile::new()),
        "prune" => Box::new(prune::Prune::new()),
        "publish" => Box::new(publish::Publish::new()),
        "query" => Box::new(query::Query::new()),
        "rebuild" => Box::new(rebuild::Rebuild::new()),
        "repo" => Box::new(repo::Repo::new()),
        "restart" => Box::new(restart::Restart::new()),
        "root" => Box::new(root::Root::new()),
        "run" => Box::new(run::Run::new()),
        "sbom" => Box::new(sbom::Sbom::new()),
        "search" => Box::new(search::Search::new()),
        "set" => Box::new(set::Set::new()),
        "stage" => Box::new(stage::Stage::new()),
        "start" => Box::new(start::Start::new()),
        "stop" => Box::new(stop::Stop::new()),
        "team" => Box::new(team::Team::new()),
        "test" => Box::new(test::Test::new()),
        "token" => Box::new(token::Token::new()),
        "trust" => Box::new(trust::Trust::new()),
        "undeprecate" => Box::new(undeprecate::Undeprecate::new()),
        "uninstall" => Box::new(uninstall::Uninstall::new()),
        "unpublish" => Box::new(unpublish::Unpublish::new()),
        "update" => Box::new(update::Update::new()),
        "version" => Box::new(version::Version::new()),
        "view" => Box::new(view::View::new()),
        "whoami" => Box::new(whoami::Whoami::new()),
        other => return Err(crate::utils::errors::unknown_command(other)),
    })
}

/// Shared by all commands: rejects unknown config from `.npmrc` unless the
/// command opts out, then logs remaining warnings.
pub fn validate_common(npm: &dyn crate::npm::Npm, meta: &CommandMeta) -> Result<()> {
    if meta.skip_config_validation {
        return Ok(());
    }
    let strict = npm.store().get_bool("strict-npmrc");
    let mut file_unknowns = Vec::new();
    for where_ in [
        ConfigSource::Builtin,
        ConfigSource::Project,
        ConfigSource::User,
        ConfigSource::Global,
    ] {
        file_unknowns.extend(npm.store().get_unknown_configs(where_));
    }
    if strict && !file_unknowns.is_empty() {
        let lines = file_unknowns
            .iter()
            .map(|u| format!("  - {} config \"{}\" from {}", u.where_.as_str(), u.key, u.source))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(NpmError::new(format!(
            "Unknown npm configuration key{}:\n{lines}\nSee `npm help npmrc` for supported config options.",
            if file_unknowns.len() > 1 { "s" } else { "" }
        ))
        .with_code("EUNKNOWNCONFIG"));
    }
    npm.store().log_warnings();
    Ok(())
}

/// Converts a JSON flag value into a plain string for command use.
pub fn flag_string(flags: &ParsedFlags, key: &str) -> Option<String> {
    match flags.flags.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Bool(true)) => Some(String::new()),
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(other) => Some(other.to_string()),
    }
}
