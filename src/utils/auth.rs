//! Port of `lib/utils/auth.js`: one-time-password retry wrappers and login.

use serde_json::Value;

use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};

/// Approximate host of `registry`, used to look up credentials.
pub fn registry_host(registry: &str) -> String {
    url::Url::parse(registry)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| registry.to_string())
}

/// Returns the `otp` option configured for `registry`, if any.
pub fn otp_from_config(npm: &dyn Npm) -> Option<String> {
    let registry = npm.store().get_str("registry");
    let host = registry_host(&registry);
    let key = format!("//{host}/:otp");
    npm.store()
        .get(&key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Runs `request` with `opts` and, when an `EOTP` error carries a web auth
/// URL, retries with the resolved one-time password.
///
/// The interactive prompts live in [`crate::utils::read_user_info`]; callers
/// that already have an OTP (from `--otp`) pass it through `opts`.
pub fn otplease<R>(
    npm: &dyn Npm,
    opts: &[(String, Value)],
    request: impl Fn(&[(String, Value)]) -> Result<R>,
) -> Result<R> {
    match request(opts) {
        Ok(value) => Ok(value),
        Err(err) if err.code.as_deref() == Some("EOTP") => {
            let otp = otp_from_config(npm).ok_or(err)?;
            let mut retry = opts.to_vec();
            retry.push(("otp".into(), Value::String(otp)));
            request(&retry)
        }
        Err(err) => Err(err),
    }
}

/// Credentials produced by a successful login.
#[derive(Debug, Clone)]
pub struct LoginCreds {
    pub message: String,
    pub token: String,
}

/// Logs in with a username/password pair against the couch login endpoint.
///
/// TODO: perform the `PUT /-/user/org.couchdb.user:<name>` request once the
/// registry client exists.
pub fn login(npm: &dyn Npm, username: &str, password: &str) -> Result<LoginCreds> {
    let opts = npm.flat_options();
    let registry = opts
        .get("registry")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let scope = opts
        .get("scope")
        .and_then(Value::as_str)
        .map(str::to_string);
    let _ = (username, password);

    let message = match scope {
        Some(scope) if !scope.is_empty() => format!("Logged in to scope {scope} on {registry}."),
        _ => format!("Logged in on {registry}."),
    };
    Err(NpmError::new("Registry login is not implemented yet").with_code("ENOTSUP")).map(|_: ()| {
        LoginCreds {
            message,
            token: String::new(),
        }
    })
}
