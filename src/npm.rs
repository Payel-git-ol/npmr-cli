//! Port of `lib/npm.js`. The `Npm` struct owns config, display, log files and
//! timers, loads config + argv, routes to a command and turns errors into the
//! formatted output users see. Commands talk to it through the [`Npm`] trait so
//! they stay decoupled from I/O details.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::{Map, Value};

use crate::store::{ConfigSource, Store};
use crate::utils::cmd_list::deref;
use crate::utils::display::{Display, DisplayOptions};
use crate::utils::errors::{NpmError, Result, unknown_command};
use crate::utils::log::Level;
use crate::utils::log_file::LogFiles;
use crate::utils::output_error::{OutputError, output_error};
use crate::utils::redact::redact_log;
use crate::utils::timers::Timers;

/// The read-only surface every command relies on.
pub trait Npm: Send + Sync {
    fn version(&self) -> &'static str {
        crate::VERSION
    }
    fn store(&self) -> &Store;
    fn store_mut(&mut self) -> &mut Store;
    fn display(&self) -> &Display;
    fn loaded(&self) -> bool;
    fn command(&self) -> Option<&str>;
    fn argv(&self) -> &[String];
    fn title(&self) -> &str;
    fn npm_root(&self) -> &Path;
    fn log_files(&self) -> &[PathBuf];

    fn flat_options(&self) -> Map<String, Value> {
        let mut flat = Map::new();
        for (key, value) in self.store().flat() {
            flat.insert(key, value);
        }
        flat.insert("nodeVersion".into(), Value::String("rust".into()));
        flat.insert(
            "npmVersion".into(),
            Value::String(self.version().to_string()),
        );
        if let Some(command) = self.command() {
            flat.insert("npmCommand".into(), Value::String(command.to_string()));
        }
        flat
    }

    fn color(&self) -> bool {
        matches!(
            self.flat_options().get("color").and_then(Value::as_str),
            Some("always")
        )
    }

    fn log_color(&self) -> bool {
        self.color()
    }

    fn global(&self) -> bool {
        self.store().global()
    }

    fn silent(&self) -> bool {
        self.store().get_str("loglevel") == "silent"
    }

    fn lockfile_version(&self) -> u8 {
        2
    }

    fn json(&self) -> bool {
        self.store().get_bool("json")
    }

    fn global_dir(&self) -> PathBuf {
        self.store().global_dir()
    }

    fn local_dir(&self) -> PathBuf {
        self.store().local_dir()
    }

    fn local_package(&self) -> Option<&Path> {
        self.store().local_package.as_deref()
    }

    fn local_prefix(&self) -> &Path {
        &self.store().local_prefix
    }

    fn global_prefix(&self) -> &Path {
        &self.store().global_prefix
    }

    fn dir(&self) -> PathBuf {
        self.store().dir()
    }

    fn bin(&self) -> PathBuf {
        self.store().bin()
    }

    fn prefix(&self) -> PathBuf {
        self.store().prefix()
    }

    fn usage(&self) -> String
    where
        Self: Sized,
    {
        crate::utils::npm_usage::usage(self)
    }

    /// Resolves a command name, honouring aliases and abbreviations.
    fn cmd(&self, name: &str) -> Result<&'static str> {
        let deref = deref(name);
        deref
            .as_deref()
            .and_then(crate::commands::canonical)
            .ok_or_else(|| unknown_command(name))
    }

    /// Environment exported to lifecycle and shellout children.
    fn child_env(&self) -> &std::collections::BTreeMap<String, String>;

    /// Runs a nested command, e.g. `restart` → `run restart`.
    fn exec(&mut self, name: &str, args: &[String]) -> Result<()>;
}

pub struct NpmCli {
    store: Store,
    display: Display,
    log_file: LogFiles,
    timers: Timers,
    argv: Vec<String>,
    argv_clean: Vec<String>,
    run_id: String,
    title: String,
    npm_root: PathBuf,
    update_notification: Option<String>,
    command: Option<String>,
    loaded: bool,
    child_env: std::collections::BTreeMap<String, String>,
}

impl NpmCli {
    pub fn new(argv: Vec<String>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let npm_root = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| cwd.clone());
        Self {
            store: Store::new(cwd, argv.clone()),
            display: Display::new(),
            log_file: LogFiles::new(),
            timers: Timers::new(),
            argv,
            argv_clean: Vec::new(),
            run_id: Utc::now().format("%Y-%m-%dT%H_%M_%S_%3fZ").to_string(),
            title: "npm".into(),
            npm_root,
            update_notification: None,
            command: None,
            loaded: false,
            child_env: std::collections::BTreeMap::new(),
        }
    }

    pub fn update_notification(&self) -> Option<&str> {
        self.update_notification.as_deref()
    }

    pub fn set_update_notification(&mut self, message: Option<String>) {
        self.update_notification = message;
    }

    /// Environment variables exported to lifecycle and shellout children
    /// (`npm_command`, `npm_lifecycle_event`, ...).
    pub fn child_env(&self) -> &std::collections::BTreeMap<String, String> {
        &self.child_env
    }

    pub fn set_child_env(&mut self, key: &str, value: String) {
        self.child_env.insert(key.to_string(), value);
    }

    pub fn timers(&mut self) -> &mut Timers {
        &mut self.timers
    }

    /// Loads config and derives the command to run.
    pub fn load(&mut self) -> Result<LoadResult> {
        self.store.load()?;

        if self.store.get_bool("versions") {
            self.argv = vec!["version".into()];
            self.store
                .set(Value::Bool(false), "usage", ConfigSource::Cli);
        } else {
            self.argv = self.store.parsed_argv.remain.clone();
        }

        let command_arg = self.argv.first().cloned();

        let loglevel = Level::from_str_lossy(&self.store.get_str("loglevel")).ok_or_else(|| {
            let bad = self.store.get_str("loglevel");
            NpmError::new(format!("Invalid loglevel: {bad}")).with_code("EBADLOGFN")
        })?;

        let color = self.store.get_str("color");
        let stdout_color = match color.as_str() {
            "always" => true,
            "never" => false,
            _ => self.display.color_supported(true),
        };
        let stderr_color = match color.as_str() {
            "always" => true,
            "never" => false,
            _ => self.display.color_supported(false),
        };

        self.display.load(DisplayOptions {
            loglevel,
            stdout_color,
            stderr_color,
            timing: self.store.get_bool("timing"),
            unicode: self.store.get_bool("unicode"),
            progress: self.store.get_bool("progress"),
            json: self.store.get_bool("json"),
            heading: self.store.get_str("heading"),
        });
        crate::utils::log::set_color(stderr_color);
        crate::utils::log::set_level(loglevel);

        if self.store.get_bool("json") && !matches!(command_arg.as_deref(), Some("ls" | "ll")) {
            crate::output::set_meta(true);
        }

        if self.store.get_bool("version") {
            crate::utils::log::standard(self.version());
            self.loaded = true;
            return Ok(LoadResult {
                exec: false,
                command: None,
                args: Vec::new(),
            });
        }

        if let Some(where_) = self.store.find("extension-file") {
            if !matches!(
                where_,
                ConfigSource::Cli | ConfigSource::Project | ConfigSource::Default
            ) {
                return Err(NpmError::new(format!(
                    "`extension-file` may only be set in project config or on the command line, not from {} config",
                    where_.as_str()
                ))
                .with_code("ENPMEXTENSIONCONFIG"));
            }
        }

        let _ = std::fs::create_dir_all(self.store.cache());
        let logs_dir = self.logs_dir();
        if self.store.get_int("logs-max") > 0 {
            let _ = std::fs::create_dir_all(logs_dir);
        }

        self.title = std::iter::once("npm".to_string())
            .chain(
                self.argv
                    .iter()
                    .skip(1)
                    .map(|a| crate::utils::redact::redact(a)),
            )
            .collect::<Vec<_>>()
            .join(" ");
        self.argv_clean = redact_log(&self.argv);
        crate::utils::log::verbose("title", &[&self.title]);
        crate::utils::log::verbose("argv", &[&self.argv_clean.join(" ")]);

        if let Some(scope) = self
            .store
            .get("scope")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            if !scope.is_empty() && !scope.starts_with('@') {
                self.store.set(
                    Value::String(format!("@{scope}")),
                    "scope",
                    ConfigSource::Cli,
                );
            }
        }

        if self.store.get_bool("force") {
            crate::utils::log::warn("using --force", &[&"Recommended protections disabled."]);
        }

        self.loaded = true;
        Ok(LoadResult {
            exec: true,
            command: command_arg,
            args: self.argv.iter().skip(1).cloned().collect(),
        })
    }

    /// Executes the command named `cmd` with `args`, routing to the matching
    /// [`Command`] implementation.
    pub fn run(&mut self, cmd: &str, args: &[String]) -> Result<()> {
        let canonical = self.cmd(cmd)?;
        self.command = Some(canonical.to_string());
        self.child_env
            .insert("npm_command".into(), canonical.to_string());
        let mut command = crate::commands::create(canonical)?;
        command.exec(self, args)
    }

    pub fn unload(&mut self) {
        self.timers.off();
        self.display.off();
        self.log_file.off();
    }

    pub fn finish(&mut self) {
        self.timers.finish(
            &self.run_id,
            &self.argv_clean,
            &self
                .log_file
                .files()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            self.version(),
        );
        self.display.flush();
        crate::output::flush();
    }

    pub fn exit_error_message(&self) -> String {
        if !self.log_file.files().is_empty() {
            let files = self
                .log_file
                .files()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", ");
            return format!("A complete log of this run can be found in: {files}");
        }
        let logs_max = self.store.get_int("logs-max");
        if logs_max <= 0 {
            return format!("Log files were not written due to the config logs-max={logs_max}");
        }
        format!(
            "Log files were not written due to an error writing to the directory: {}\nYou can rerun the command with `--loglevel=verbose` to see the logs in your terminal",
            self.logs_dir().to_string_lossy()
        )
    }

    /// Formats `err` for display and returns the populated error.
    pub fn handle_error(&mut self, err: &mut NpmError) -> OutputError {
        let message = crate::utils::error_message::get_error(err, self);
        for (file, content) in &message.files {
            let file_path = format!("{}{}", self.log_path().to_string_lossy(), file);
            let log_files: Vec<String> = self
                .log_file
                .files()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let file_content = format!(
                "Log files:\n{}\n\n{}\n",
                log_files.join("\n"),
                content.trim()
            );
            match std::fs::write(&file_path, file_content) {
                Ok(()) => crate::utils::log::verbose(
                    "",
                    &[&format!("\n\nFor a full report see:\n{file_path}")],
                ),
                Err(file_err) => crate::utils::log::warn(
                    "",
                    &[&format!(
                        "Could not write error message to {file} due to {file_err}"
                    )],
                ),
            }
        }
        let out = OutputError {
            standard: Vec::new(),
            verbose: Vec::new(),
            error: Vec::new(),
            summary: message.summary.iter().map(|(_, b)| b.clone()).collect(),
            detail: message.detail.iter().map(|(_, b)| b.clone()).collect(),
        };
        output_error(&out);
        out
    }

    fn logs_dir(&self) -> PathBuf {
        match self.store.get("logs-dir").and_then(Value::as_str) {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => self.store.cache().join("_logs"),
        }
    }

    pub fn log_path(&self) -> PathBuf {
        self.logs_dir().join(format!("{}-", self.run_id))
    }
}

impl Npm for NpmCli {
    fn store(&self) -> &Store {
        &self.store
    }
    fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }
    fn display(&self) -> &Display {
        &self.display
    }
    fn loaded(&self) -> bool {
        self.loaded
    }
    fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }
    fn argv(&self) -> &[String] {
        &self.argv
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn npm_root(&self) -> &Path {
        &self.npm_root
    }
    fn log_files(&self) -> &[PathBuf] {
        self.log_file.files()
    }
    fn child_env(&self) -> &std::collections::BTreeMap<String, String> {
        &self.child_env
    }
    fn exec(&mut self, name: &str, args: &[String]) -> Result<()> {
        self.run(name, args)
    }
}

#[derive(Debug, Clone)]
pub struct LoadResult {
    pub exec: bool,
    pub command: Option<String>,
    pub args: Vec<String>,
}
