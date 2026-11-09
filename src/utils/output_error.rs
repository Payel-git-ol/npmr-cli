//! Port of `lib/utils/output-error.js`.

use crate::utils::errors::NpmError;
use serde_json::Value;

/// Splits a parsed error into the streams npm uses to report it:
/// stdout lines, verbose log lines and stderr lines.
#[derive(Debug, Default, Clone)]
pub struct OutputError {
    pub standard: Vec<String>,
    pub verbose: Vec<(String, String)>,
    pub error: Vec<(String, String)>,
    pub summary: Vec<String>,
    pub detail: Vec<String>,
}

pub fn output_error(err: &OutputError) {
    for line in &err.standard {
        crate::utils::log::standard(line);
    }
    for (prefix, line) in &err.verbose {
        crate::utils::log::verbose(prefix, &[line]);
    }
    for (prefix, line) in &err.error {
        crate::utils::log::error(prefix, &[line]);
    }
    for line in &err.summary {
        crate::utils::log::error("", &[line]);
    }
    for line in &err.detail {
        crate::utils::log::error("", &[line]);
    }
}

/// Serialisable form of an error for `--json` runs.
pub fn json_error(err: Option<&NpmError>, loaded: bool, json: bool) -> Option<Value> {
    let err = err?;
    if !loaded || !json {
        return None;
    }
    Some(serde_json::json!({
        "code": err.code,
        "summary": err.summary.join("\n").trim(),
        "detail": err.detail.join("\n").trim(),
    }))
}
