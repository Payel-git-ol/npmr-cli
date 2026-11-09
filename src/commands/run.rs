//! Port of `lib/commands/run.js`: lists or executes package scripts.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::arborist_cmd::WorkspaceContext;
use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::is_windows::is_windows_shell;

pub static META: CommandMeta = CommandMeta::new("run", "Run arbitrary package scripts")
    .with_params(&[
        "workspace",
        "workspaces",
        "include-workspace-root",
        "if-present",
        "ignore-scripts",
        "foreground-scripts",
        "script-shell",
    ])
    .with_usage(&["<command> [-- <args>]"])
    .workspaces()
    .shellout();

/// Commands npm itself runs, listed first without `npm run` in `--parseable`.
const LIFECYCLE_ORDER: &[&str] = &[
    "prepare",
    "prepublishOnly",
    "prepack",
    "postpack",
    "dependencies",
    "preinstall",
    "install",
    "postinstall",
    "prepublish",
    "publish",
    "postpublish",
    "prerestart",
    "restart",
    "postrestart",
    "prestart",
    "start",
    "poststart",
    "prestop",
    "stop",
    "poststop",
    "pretest",
    "test",
    "posttest",
    "preuninstall",
    "uninstall",
    "postuninstall",
    "preversion",
    "version",
    "postversion",
];

pub struct RunScript;

impl RunScript {
    pub fn new() -> Self {
        Self
    }

    /// Reads and parses `package.json` at `path`; missing files become `{}`.
    pub fn read_package(path: &Path) -> Value {
        std::fs::read_to_string(path.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
    }
}

impl Default for RunScript {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the shell command for one event, honouring `--script-shell`.
pub fn event_command(
    npm: &dyn Npm,
    path: &Path,
    manifest: &Value,
    event: &str,
    args: &[String],
) -> Result<String> {
    let scripts = manifest.get("scripts").and_then(Value::as_object);
    let mut scripts = scripts.cloned().unwrap_or_default();
    if event == "restart" && !scripts.contains_key("restart") {
        scripts.insert(
            "restart".into(),
            Value::String("npm stop --if-present && npm start".into()),
        );
    } else if event == "env" && !scripts.contains_key("env") {
        scripts.insert(
            "env".into(),
            Value::String(if is_windows_shell() { "SET" } else { "env" }.into()),
        );
    }

    let Some(script) = scripts.get(event).and_then(Value::as_str) else {
        if npm.store().get_bool("if-present") {
            return Err(NpmError::new(format!("__MISSING__{event}")));
        }
        let suggestions = crate::utils::did_you_mean::did_you_mean(
            Some(&Value::Object({
                let mut map = serde_json::Map::new();
                map.insert("scripts".into(), Value::Object(scripts.clone()));
                map
            })),
            event,
        );
        return Err(NpmError::new(format!(
            "Missing script: \"{event}\"{suggestions}\n\nTo see a list of scripts, run:\n  npm run"
        )));
    };

    let mut command = script.to_string();
    if !args.is_empty() {
        command.push_str(" --");
        for arg in args {
            command.push(' ');
            command.push_str(&shell_quote(arg));
        }
    }
    let _ = path;
    Ok(command)
}

fn shell_quote(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:@=+".contains(c))
    {
        arg.to_string()
    } else if cfg!(windows) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

/// Events to run for `event`: pre, the event with args, then post.
pub fn event_chain(manifest: &Value, event: &str, ignore_scripts: bool) -> Vec<(String, bool)> {
    let scripts = manifest.get("scripts").and_then(Value::as_object);
    let has = |name: &str| scripts.is_some_and(|s| s.contains_key(name));
    let mut events = vec![(event.to_string(), true)];
    if !ignore_scripts {
        if has(&format!("pre{event}")) {
            events.insert(0, (format!("pre{event}"), false));
        }
        if has(&format!("post{event}")) {
            events.push((format!("post{event}"), false));
        }
    }
    events
}

impl RunScript {
    fn run_event(
        &self,
        npm: &dyn Npm,
        path: &Path,
        manifest: &Value,
        args: &[String],
    ) -> Result<()> {
        let Some(event) = args.first() else {
            return Ok(());
        };
        let extra = &args[1..];
        let chain = event_chain(manifest, event, npm.store().get_bool("ignore-scripts"));
        for (ev, with_args) in chain {
            let ev_args: &[String] = if with_args { extra } else { &[] };
            match event_command(npm, path, manifest, &ev, ev_args) {
                Ok(command) => run_shell(npm, path, &command)?,
                Err(err) if err.body.starts_with("__MISSING__") => {
                    if npm.store().get_bool("if-present") {
                        return Ok(());
                    }
                    return Err(NpmError::new(err.body.trim_start_matches("__MISSING__")));
                }
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    fn list(&self, npm: &dyn Npm, path: &Path, workspace: Option<&str>) -> bool {
        let manifest = Self::read_package(path);
        let Some(scripts) = manifest.get("scripts").and_then(Value::as_object) else {
            return false;
        };
        if scripts.is_empty() || npm.silent() {
            return false;
        }
        if npm.json() {
            let value = match workspace {
                Some(name) => serde_json::json!({ name: scripts }),
                None => Value::Object(scripts.clone()),
            };
            crate::output::set_meta(true);
            crate::output::standard(value.to_string());
            return true;
        }
        if npm.store().get_bool("parseable") {
            let lines: Vec<String> = scripts
                .iter()
                .map(|(k, v)| match workspace {
                    Some(ws) => format!("{ws}:{k}:{}", v.as_str().unwrap_or_default()),
                    None => format!("{k}:{}", v.as_str().unwrap_or_default()),
                })
                .collect();
            crate::output::standard(lines.join("\n"));
            return true;
        }

        let name = manifest
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut lifecycles = Vec::new();
        let mut plain = Vec::new();
        let has_lifecycles = scripts
            .keys()
            .any(|k| LIFECYCLE_ORDER.contains(&k.as_str()));
        for (key, value) in scripts {
            if LIFECYCLE_ORDER.contains(&key.as_str()) {
                lifecycles.push((key, value));
            } else {
                plain.push((key, value));
            }
        }
        if !lifecycles.is_empty() {
            crate::output::standard(format!("Lifecycle scripts included in {name}:"));
            for (key, value) in lifecycles {
                crate::output::standard(format!("  {key}"));
                crate::output::standard(format!("    {}", value.as_str().unwrap_or_default()));
            }
        }
        if !plain.is_empty() {
            if !has_lifecycles {
                crate::output::standard(format!(
                    "Scripts available in {name} via `npm run-script`:"
                ));
            } else {
                crate::output::standard("available via `npm run-script`:");
            }
            for (key, value) in plain {
                crate::output::standard(format!("  {key}"));
                crate::output::standard(format!("    {}", value.as_str().unwrap_or_default()));
            }
        }
        true
    }
}

/// Runs `command` through the configured shell in `path`.
pub fn run_shell(npm: &dyn Npm, path: &Path, command: &str) -> Result<()> {
    let shell = npm.store().get_str("script-shell");
    let mut cmd = if shell.is_empty() {
        if cfg!(windows) {
            let mut cmd = std::process::Command::new("cmd");
            cmd.arg("/d").arg("/s").arg("/c").arg(command);
            cmd
        } else {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(command);
            cmd
        }
    } else {
        let mut cmd = std::process::Command::new(shell.as_ref());
        if cfg!(windows) {
            cmd.arg("/d").arg("/s").arg("/c").arg(command);
        } else {
            cmd.arg("-c").arg(command);
        }
        cmd
    };
    cmd.current_dir(path);
    for (key, value) in npm.child_env() {
        cmd.env(key, value);
    }
    let status = cmd.status().map_err(NpmError::from)?;
    if !status.success() {
        let code = status.code().unwrap_or(1);
        return Err(NpmError::new(format!(
            "Lifecycle script failed with error:\n  command: {command}\n  cwd: {}",
            path.display()
        ))
        .with_code(format!("ELIFECYCLE {code}")));
    }
    Ok(())
}

impl Command for RunScript {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let prefix = npm.local_prefix().to_path_buf();
        if args.is_empty() {
            self.list(npm, &prefix, None);
            return Ok(());
        }
        let manifest = Self::read_package(&prefix);
        self.run_event(npm, &prefix, &manifest, args)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let prefix = npm.local_prefix().to_path_buf();
        let ctx = WorkspaceContext::new(npm, workspaces.clone());
        let all: Vec<(String, String)> = workspaces.into_iter().collect();
        for (i, (name, path)) in all.iter().enumerate() {
            let last = i + 1 == all.len();
            let path = PathBuf::from(path);
            if args.is_empty() {
                let printed = self.list(&ctx, &path, Some(name));
                if printed && !last {
                    crate::output::standard("");
                }
                continue;
            }
            let manifest = Self::read_package(&path);
            if let Err(err) = self.run_event(&ctx, &path, &manifest, args) {
                let label = manifest.get("name").and_then(Value::as_str).unwrap_or(name);
                crate::output::error(format!(
                    "Lifecycle script `{}` failed with error:\n{}\nworkspace: {label}\nlocation: {}",
                    args[0],
                    err.body,
                    path.display()
                ));
                if !last {
                    crate::output::error("");
                }
            }
        }
        let _ = prefix;
        Ok(())
    }
}

impl RunScript {
    pub fn command_names(manifest: &Value) -> Vec<String> {
        manifest
            .get("scripts")
            .and_then(Value::as_object)
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_adds_pre_and_post() {
        let manifest = serde_json::json!({"scripts": {"pretest": "echo pre", "test": "t", "posttest": "echo post"}});
        let chain = event_chain(&manifest, "test", false);
        assert_eq!(
            chain,
            vec![
                ("pretest".to_string(), false),
                ("test".to_string(), true),
                ("posttest".to_string(), false),
            ]
        );
        let chain = event_chain(&manifest, "test", true);
        assert_eq!(chain, vec![("test".to_string(), true)]);
    }

    #[test]
    fn restart_gets_default_script() {
        let manifest = serde_json::json!({"scripts": {}});
        let npm = crate::npm::NpmCli::new(vec!["npm".into()]);
        let command = event_command(&npm, Path::new("."), &manifest, "restart", &[]).unwrap();
        assert_eq!(command, "npm stop --if-present && npm start");
    }
}
