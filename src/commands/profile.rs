//! Port of `lib/commands/profile.js`. Subcommand validation is ported;
//! registry calls (`npm-profile`) report `ENOTSUP`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new(
    "profile",
    "Change settings on your registry profile",
)
.with_params(&["registry", "json", "parseable", "otp"])
.with_usage(&[
    "enable-2fa [auth-only|auth-and-writes]",
    "disable-2fa",
    "get [<key>]",
    "set <key> <value>",
])
.skip_validation();

pub struct Profile;

impl Profile {
    pub fn new() -> Self {
        Self
    }

    fn enable_2fa(&self, args: &[String]) -> Result<()> {
        if args.len() > 1 {
            return Err(NpmError::new("npm profile enable-2fa [auth-and-writes|auth-only]"));
        }
        let mode = args.first().map(String::as_str).unwrap_or("auth-and-writes");
        if mode != "auth-only" && mode != "auth-and-writes" {
            return Err(NpmError::new(format!(
                "Invalid two-factor authentication mode \"{mode}\".\n\
                 Valid modes are:\n  \
                 auth-only - Require two-factor authentication only when logging in\n  \
                 auth-and-writes - Require two-factor authentication when logging in\n  \
                 AND when publishing"
            )));
        }
        Err(registry_stub("enabling two-factor authentication"))
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

fn registry_stub(action: &str) -> NpmError {
    NpmError::new(format!(
        "Cannot {action}: registry access is not implemented yet"
    ))
    .with_code("ENOTSUP")
}

impl Command for Profile {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let Some(subcmd) = args.first().filter(|a| !a.is_empty()) else {
            return Err(META.usage_error(""));
        };
        let rest = &args[1..];
        match subcmd.as_str() {
            "enable-2fa" | "enable-tfa" | "enable2fa" | "enabletfa" => self.enable_2fa(rest),
            "disable-2fa" | "disable-tfa" | "disable2fa" | "disabletfa" => {
                Err(registry_stub("disabling two-factor authentication"))
            }
            "get" => Err(registry_stub("reading your profile")),
            "set" => {
                let key = rest
                    .first()
                    .map(|k| k.to_lowercase().trim().to_string())
                    .unwrap_or_default();
                if key.is_empty() {
                    return Err(registry_stub("updating your profile"));
                }
                let allowed = ["email", "password", "fullname", "homepage", "github"];
                if !allowed.contains(&key.as_str()) {
                    return Err(NpmError::new(format!(
                        "Can't set \"{key}\" in your profile. Only the following keys can be set: {}",
                        allowed.join(", ")
                    )));
                }
                Err(registry_stub("updating your profile"))
            }
            other => Err(NpmError::new(format!("Unknown profile command: {other}"))),
        }
    }
}
