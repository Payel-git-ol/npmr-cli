//! Port of `lib/utils/get-identity.js`.

use crate::utils::errors::{NpmError, Result};

/// Reads the username for `registry` from the config store, falling back to
/// `/-/whoami` when only a token is configured. TODO: perform the request
/// once a registry client is available.
pub fn get_identity(npm: &dyn crate::npm::Npm) -> Result<String> {
    let registry = npm.store().get_str("registry");
    let key = format!("//{}/:_authToken", strip_scheme(&registry));
    let bare_key = registry
        .split_once("//")
        .map(|(_, rest)| rest)
        .unwrap_or(registry.as_ref());
    let username_key = format!("//{bare_key}/:username");

    if let Some(username) = npm.store().get(&username_key).and_then(|v| v.as_str()) {
        if !username.is_empty() {
            return Ok(username.to_string());
        }
    }

    if npm.store().get(&key).and_then(|v| v.as_str()).is_some() {
        // TODO: fetch `/-/whoami` with the configured token.
        return Err(NpmError::new(format!(
            "Unable to determine the logged in user for {registry} without a registry request"
        ))
        .with_code("ENEEDAUTH"));
    }

    Err(NpmError::new("This command requires you to be logged in.").with_code("ENEEDAUTH"))
}

fn strip_scheme(registry: &str) -> &str {
    registry
        .strip_prefix("https://")
        .or_else(|| registry.strip_prefix("http://"))
        .unwrap_or(registry)
}
