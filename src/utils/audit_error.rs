//! Port of `lib/utils/audit-error.js`: prints an audit endpoint error (or
//! nothing when the report has none) and reports whether one was present.

use serde_json::{json, Value};

use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::warn;
use crate::utils::redact::redact;

/// The audit report, reduced to its `error` member.
#[derive(Debug, Clone, Default)]
pub struct AuditReport {
    pub error: Option<AuditEndpointError>,
}

#[derive(Debug, Clone, Default)]
pub struct AuditEndpointError {
    pub message: String,
    pub method: Option<String>,
    pub uri: Option<String>,
    pub headers: Option<Value>,
    pub status_code: Option<u16>,
    pub body: String,
}

/// Returns whether the report carried an error. Outside the `audit` command
/// the error is only reported, not printed.
pub fn audit_error(npm: &dyn Npm, report: &AuditReport, command: &str) -> Result<bool> {
    let Some(error) = &report.error else {
        return Ok(false);
    };
    if command != "audit" {
        return Ok(true);
    }

    warn("audit", &[&error.message]);
    if npm.json() {
        let payload = json!({
            "message": error.message,
            "method": error.method,
            "uri": error.uri.as_deref().map(redact),
            "headers": error.headers,
            "statusCode": error.status_code,
            "body": error.body,
        });
        crate::output::standard(&format!("{}\n", serde_json::to_string(&payload).unwrap_or_default()));
    } else {
        crate::output::standard(&format!("{}\n", error.body));
    }
    Err(NpmError::new("audit endpoint returned an error"))
}
