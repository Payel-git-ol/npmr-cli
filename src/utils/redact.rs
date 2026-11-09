//! Replacement for `@npmcli/redact`. Best-effort removal of secrets from
//! strings before they are written to logs or shown in process titles.

use regex::Regex;
use std::sync::OnceLock;

static BASIC_AUTH: OnceLock<Regex> = OnceLock::new();
static NPM_TOKEN: OnceLock<Regex> = OnceLock::new();
static BEARER: OnceLock<Regex> = OnceLock::new();
static AUTH_HEADER: OnceLock<Regex> = OnceLock::new();

fn basic_auth() -> &'static Regex {
    BASIC_AUTH.get_or_init(|| {
        Regex::new(r"(?i)(https?://)[^/@\s]+(?::[^/@\s]*)?@").expect("valid basic auth regex")
    })
}

fn npm_token() -> &'static Regex {
    NPM_TOKEN.get_or_init(|| {
        Regex::new(r"\b(?:_authToken|npm_[A-Za-z0-9]{36}|//registry[^:\s]*:_authToken)\b[^ ]*")
            .expect("valid token regex")
    })
}

fn bearer() -> &'static Regex {
    BEARER.get_or_init(|| Regex::new(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]+").expect("valid bearer"))
}

fn auth_header() -> &'static Regex {
    AUTH_HEADER.get_or_init(|| {
        Regex::new(r"(?i)(authorization\s*[:=]\s*)\S+").expect("valid auth header regex")
    })
}

pub fn redact(input: &str) -> String {
    let out = basic_auth().replace_all(input, "${1}***:***@");
    let out = auth_header().replace_all(&out, "${1}[REDACTED]");
    let out = bearer().replace_all(&out, "[REDACTED]");
    npm_token().replace_all(&out, "[REDACTED]").into_owned()
}

/// Redacts every element of an argument list. Used for the cooked argv that
/// npm logs for debugging and for the process title.
pub fn redact_log(args: &[String]) -> Vec<String> {
    args.iter().map(|a| redact(a)).collect()
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn hides_basic_auth() {
        assert_eq!(
            redact("https://user:pass@example.com/x"),
            "https://***:***@example.com/x"
        );
    }

    #[test]
    fn hides_bearer_tokens() {
        assert!(!redact("Bearer abcdef12345").contains("abcdef12345"));
    }
}
