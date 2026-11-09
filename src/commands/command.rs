//! The [`Command`] trait replaces the `BaseCommand` class hierarchy of
//! `lib/base-cmd.js`: static metadata becomes [`CommandMeta`], flags are
//! described by [`FlagDef`] and behaviour by trait methods.

use serde_json::{Map, Value};

use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

/// A single command specific option as declared in `static definitions`.
#[derive(Debug, Clone)]
pub struct FlagDef {
    pub key: &'static str,
    pub short: Option<char>,
    pub aliases: &'static [&'static str],
    pub ty: FlagType,
    pub default: Option<Value>,
    pub description: &'static str,
    pub usage: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagType {
    Boolean,
    String,
    Number,
    List,
}

impl FlagDef {
    pub const fn new(key: &'static str, ty: FlagType, description: &'static str) -> Self {
        Self {
            key,
            short: None,
            aliases: &[],
            ty,
            default: None,
            description,
            usage: key,
            required: false,
        }
    }

    pub const fn with_short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }

    pub const fn with_usage(mut self, usage: &'static str) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn with_aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// Static command metadata, the Rust equivalent of `static` class fields.
#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static [&'static str],
    /// Names of global config params the command accepts (resolved against
    /// the shared definitions table).
    pub params: &'static [&'static str],
    pub definitions: &'static [FlagDef],
    pub positionals: Option<usize>,
    pub workspaces: bool,
    pub ignore_implicit_workspace: bool,
    pub check_dev_engines: bool,
    pub is_shellout: bool,
    pub skip_config_validation: bool,
    pub is_arborist_cmd: bool,
}

impl CommandMeta {
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            usage: &[""],
            params: &[],
            definitions: &[],
            positionals: None,
            workspaces: false,
            ignore_implicit_workspace: true,
            check_dev_engines: false,
            is_shellout: false,
            skip_config_validation: false,
            is_arborist_cmd: false,
        }
    }

    pub const fn with_usage(mut self, usage: &'static [&'static str]) -> Self {
        self.usage = usage;
        self
    }

    pub const fn with_params(mut self, params: &'static [&'static str]) -> Self {
        self.params = params;
        self
    }

    pub const fn with_definitions(mut self, definitions: &'static [FlagDef]) -> Self {
        self.definitions = definitions;
        self
    }

    pub const fn with_positionals(mut self, n: usize) -> Self {
        self.positionals = Some(n);
        self
    }

    pub const fn workspaces(mut self) -> Self {
        self.workspaces = true;
        self.ignore_implicit_workspace = false;
        self
    }

    pub const fn shellout(mut self) -> Self {
        self.is_shellout = true;
        self
    }

    pub const fn arborist(mut self) -> Self {
        self.is_arborist_cmd = true;
        self.check_dev_engines = true;
        self.workspaces = true;
        self.ignore_implicit_workspace = false;
        self
    }

    pub const fn skip_validation(mut self) -> Self {
        self.skip_config_validation = true;
        self
    }

    /// Renders the `--help` text the same way `BaseCommand.getUsage` does.
    pub fn usage_text(&self, parent_name: Option<&str>, include_descriptions: bool) -> String {
        let mut out = vec![self.description.to_string(), String::new(), "Usage:".into()];
        let full_name = match parent_name {
            Some(parent) => format!("{parent} {}", self.name),
            None => self.name.to_string(),
        };
        for usage in self.usage {
            out.push(format!("npm {full_name} {usage}").trim().to_string());
        }

        let defs = self.definitions;
        if !defs.is_empty() {
            let mut rendered = String::new();
            let mut current = String::new();
            for def in defs {
                let param = format!("[{}]", def.usage);
                if current.len() + param.len() > 80 {
                    if !rendered.is_empty() {
                        rendered.push('\n');
                    }
                    rendered.push_str(&current);
                    current.clear();
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(&param);
            }
            if !current.is_empty() {
                if !rendered.is_empty() {
                    rendered.push('\n');
                }
                rendered.push_str(&current);
            }
            out.push(String::new());
            out.push("Options:".into());
            out.push(rendered);
            if include_descriptions {
                out.push(String::new());
                for def in defs.iter().filter(|d| !d.description.is_empty()) {
                    let shortcut = def.short.map(|s| format!("-{s}")).unwrap_or_default();
                    let aliases = def
                        .aliases
                        .iter()
                        .map(|a| format!("--{a}"))
                        .collect::<Vec<_>>()
                        .join("|");
                    let main = format!("--{}", def.key);
                    let name = [shortcut, main, aliases]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("|");
                    let required = if def.required { " (required)" } else { "" };
                    out.push(format!("  {name}{required}"));
                    let first = def.description.lines().next().unwrap_or_default();
                    out.push(format!("    {}", first.trim()));
                    out.push(String::new());
                }
            }
        }

        let aliases = crate::utils::cmd_list::aliases_for(self.name);
        if !aliases.is_empty() {
            let plural = if aliases.len() == 1 { "" } else { "es" };
            out.push(String::new());
            out.push(format!("alias{plural}: {}", aliases.join(", ")));
        }

        out.push(String::new());
        let help_name = parent_name
            .and_then(|p| p.split(' ').next())
            .unwrap_or(self.name);
        out.push(format!("Run \"npm help {help_name}\" for more info"));
        out.join("\n")
    }

    /// The `EUSAGE` error carrying this command's help text.
    pub fn usage_error(&self, prefix: impl AsRef<str>) -> NpmError {
        let prefix = prefix.as_ref();
        let body = if prefix.is_empty() {
            format!("\n{}", self.usage_text(None, true))
        } else {
            format!("\n{prefix}\n\n{}", self.usage_text(None, true))
        };
        NpmError::new(body).with_code("EUSAGE")
    }
}

/// Parsed flags and the leftover positional arguments.
#[derive(Debug, Default, Clone)]
pub struct ParsedFlags {
    pub flags: Map<String, Value>,
    pub positionals: Vec<String>,
}

impl ParsedFlags {
    pub fn bool_flag(&self, key: &str) -> bool {
        self.flags
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn str_flag(&self, key: &str) -> Option<&str> {
        self.flags.get(key).and_then(Value::as_str)
    }
}

/// Behaviour of a single npm command.
pub trait Command: Send {
    fn meta(&self) -> &'static CommandMeta;

    /// Runs the command. `args` are the positional arguments after npm and the
    /// command name were removed.
    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let _ = (npm, args);
        Ok(())
    }

    /// Runs the command once per workspace. The default delegates to [`exec`]
    /// for commands that do not support workspaces.
    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        self.exec(npm, args)
    }

    fn parse_flags(&self, args: &[String]) -> ParsedFlags {
        parse_flags(self.meta(), args)
    }

    fn validate_cli(&self, npm: &(dyn Npm + '_), _args: &[String]) -> Result<()> {
        npm.store().log_warnings();
        Ok(())
    }
}

/// Splits `args` into flags and positionals according to `meta`.
pub fn parse_flags(meta: &CommandMeta, args: &[String]) -> ParsedFlags {
    let mut parsed = ParsedFlags::default();
    for def in meta.definitions {
        if let Some(default) = &def.default {
            parsed.flags.insert(def.key.to_string(), default.clone());
        }
    }

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let Some(long) = arg.strip_prefix("--") else {
            if let Some(short) = arg.strip_prefix('-') {
                let key = meta
                    .definitions
                    .iter()
                    .find(|d| d.short.is_some_and(|c| short.starts_with(c)));
                if let Some(def) = key {
                    match def.ty {
                        FlagType::Boolean => {
                            parsed.flags.insert(def.key.into(), Value::Bool(true));
                        }
                        _ => {
                            if let Some(value) = iter.next() {
                                parsed
                                    .flags
                                    .insert(def.key.into(), Value::String(value.clone()));
                            }
                        }
                    }
                    continue;
                }
            }
            parsed.positionals.push(arg.clone());
            continue;
        };

        let (key, inline_value) = match long.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (long, None),
        };
        let def = meta
            .definitions
            .iter()
            .find(|d| d.key == key || d.aliases.contains(&key));
        let Some(def) = def else {
            parsed.positionals.push(arg.clone());
            continue;
        };
        match def.ty {
            FlagType::Boolean => {
                parsed
                    .flags
                    .insert(def.key.into(), Value::Bool(inline_value != Some("false")));
            }
            _ => {
                let value = match inline_value {
                    Some(v) => v.to_string(),
                    None => iter.next().cloned().unwrap_or_default(),
                };
                parsed.flags.insert(def.key.into(), Value::String(value));
            }
        }
    }

    if let Some(expected) = meta.positionals {
        if parsed.positionals.len() > expected {
            parsed.positionals.truncate(expected);
        }
    }
    parsed
}
