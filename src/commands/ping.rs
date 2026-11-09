//! Port of `lib/commands/ping.js`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;
use crate::utils::redact::redact;

pub static META: CommandMeta =
    CommandMeta::new("ping", "Ping npm registry").with_params(&["registry"]);

pub struct Ping;

impl Ping {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Ping {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Ping {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), _args: &[String]) -> Result<()> {
        let registry = npm.store().get_str("registry");
        let clean_registry = redact(&registry);
        crate::utils::log::notice("PING", &[&clean_registry]);
        let start = std::time::Instant::now();
        let details = crate::utils::ping::ping(&registry)?;
        let elapsed = start.elapsed().as_millis();

        crate::utils::log::notice("PONG", &[&format!("{elapsed}ms")]);
        if npm.store().get_bool("json") {
            npm.display().standard(
                &serde_json::json!({
                    "registry": clean_registry,
                    "time": elapsed,
                    "details": details,
                })
                .to_string(),
            );
        } else if !details.is_empty() {
            crate::utils::log::notice(
                "PONG",
                &[&serde_json::to_string_pretty(&details).unwrap_or_default()],
            );
        }
        Ok(())
    }
}
