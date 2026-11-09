//! Port of `lib/utils/oidc.js`: in CI, exchanges a workload identity token
//! for an npm token. HTTP is not available yet, so the exchange attempt is
//! logged and skipped; it never throws, matching the upstream contract.

use crate::utils::log::{silly, verbose};

/// The CI vendors whose identity tokens npm accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiVendor {
    GitHubActions,
    GitLab,
    CircleCI,
}

pub fn detect_ci(env: impl Fn(&str) -> Option<String>) -> Option<CiVendor> {
    if env("GITHUB_ACTIONS").as_deref() == Some("true") {
        Some(CiVendor::GitHubActions)
    } else if env("GITLAB_CI").as_deref() == Some("true") {
        Some(CiVendor::GitLab)
    } else if env("CIRCLECI").as_deref() == Some("true") {
        Some(CiVendor::CircleCI)
    } else {
        None
    }
}

/// The audience a CI identity token is requested for.
pub fn audience(registry: &str) -> Option<String> {
    let host = registry
        .split("://")
        .nth(1)
        .unwrap_or(registry)
        .split('/')
        .next()?;
    Some(format!("npm:{host}"))
}

/// The `//host/path:_authToken` configuration key for a registry.
pub fn auth_token_key(registry: &str) -> String {
    let without_scheme = registry.split("://").nth(1).unwrap_or(registry);
    let (host, path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx..]),
        None => (without_scheme, "/"),
    };
    format!("//{host}{path}:_authToken")
}

/// Performs the token exchange for `(package, id_token)`.
pub type Exchange<'f> = dyn Fn(&str, &str) -> Option<String> + 'f;

/// Runs the OIDC flow. `exchange` performs the actual token exchange; when
/// it is not wired up yet the attempt is logged and skipped.
pub fn oidc(
    package_name: &str,
    registry: &str,
    env: impl Fn(&str) -> Option<String>,
    exchange: Option<&Exchange>,
) -> Option<(String, String)> {
    let vendor = detect_ci(&env)?;
    let mut id_token = env("NPM_ID_TOKEN");

    if id_token.is_none() && vendor == CiVendor::GitHubActions {
        let has_request = env("ACTIONS_ID_TOKEN_REQUEST_URL").is_some()
            && env("ACTIONS_ID_TOKEN_REQUEST_TOKEN").is_some();
        if !has_request {
            silly(
                "oidc",
                &[&"Skipped because incorrect permissions for id-token within GitHub workflow"],
            );
            return None;
        }
        // Fetching the token needs an HTTP client, which is not ported yet.
        verbose(
            "oidc",
            &[&"Failed to fetch id_token from GitHub: HTTP client not implemented"],
        );
    }

    let id_token = id_token.take()?;
    let audience = audience(registry)?;
    let auth_token_key = auth_token_key(registry);

    let Some(exchange) = exchange else {
        verbose(
            "oidc",
            &[&format!(
                "Failed token exchange request with body message: token exchange for {package_name} (audience {audience}) is not implemented"
            )],
        );
        return None;
    };
    let Some(token) = exchange(package_name, &id_token) else {
        verbose(
            "oidc",
            &[&"Failed because token exchange was missing the token in the response body"],
        );
        return None;
    };
    verbose("oidc", &[&"Successfully retrieved and set token"]);
    Some((auth_token_key, token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_audience_and_key() {
        assert_eq!(
            audience("https://registry.npmjs.org/"),
            Some("npm:registry.npmjs.org".into())
        );
        assert_eq!(
            auth_token_key("https://registry.npmjs.org/"),
            "//registry.npmjs.org/:_authToken"
        );
        assert_eq!(
            auth_token_key("https://npm.example.com/npm/"),
            "//npm.example.com/npm/:_authToken"
        );
    }

    #[test]
    fn detects_supported_ci_only() {
        let github = |key: &str| (key == "GITHUB_ACTIONS").then(|| "true".to_string());
        assert_eq!(detect_ci(github), Some(CiVendor::GitHubActions));
        assert_eq!(detect_ci(|_| None), None);
    }

    #[test]
    fn skips_outside_ci() {
        assert!(oidc("pkg", "https://registry.npmjs.org/", |_| None, None).is_none());
    }
}
