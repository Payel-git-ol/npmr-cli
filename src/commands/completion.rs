//! Port of `lib/commands/completion.js`: prints shell completions for the
//! words on the command line being completed.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::cmd_list::{COMMANDS, aliases, deref};
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("completion", "Tab Completion for npm")
    .with_usage(&["[<command-line> ...]"])
    .skip_validation();

pub struct Completion;

impl Completion {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Completion {
    fn default() -> Self {
        Self::new()
    }
}

/// Suggested completions for a partial word: command names and aliases.
fn complete_commands(partial: &str) -> Vec<String> {
    COMMANDS
        .iter()
        .copied()
        .chain(aliases().keys().copied())
        .filter(|name| name.starts_with(partial))
        .map(str::to_string)
        .collect()
}

impl Command for Completion {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(current) = args.last() else {
            crate::output::set_meta(false);
            for name in COMMANDS {
                crate::output::standard(*name);
            }
            return Ok(());
        };

        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let command = words.iter().skip(1).find(|w| !w.starts_with('-'));
        let completions = match command.and_then(|c| deref(c)) {
            Some(cmd) if !words.contains(&cmd.as_str()) || words.last() == Some(&args[0].as_str()) => {
                complete_commands(current)
            }
            Some(cmd) => match cmd.as_str() {
                "stage" => crate::commands::stage::SUBCOMMANDS
                    .iter()
                    .filter(|s| s.starts_with(current))
                    .map(|s| s.to_string())
                    .collect(),
                "trust" => crate::commands::trust::SUBCOMMANDS
                    .iter()
                    .filter(|s| s.starts_with(current))
                    .map(|s| s.to_string())
                    .collect(),
                _ => Vec::new(),
            },
            None => complete_commands(current),
        };

        crate::output::set_meta(false);
        for line in completions {
            crate::output::standard(line);
        }
        Ok(())
    }
}
