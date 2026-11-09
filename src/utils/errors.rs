use std::fmt::Write as _;

use thiserror::Error;

/// A command line usage error. Rendered together with the usage text of the
/// command that produced it.
#[derive(Debug, Error)]
#[error("{}", .body)]
pub struct NpmError {
    pub code: Option<String>,
    pub summary: Vec<String>,
    pub detail: Vec<String>,
    pub exit_code: Option<i32>,
    pub suppress_error: bool,
    pub body: String,
}

impl NpmError {
    pub fn new(body: impl Into<String>) -> Self {
        let body = body.into();
        Self {
            summary: vec![body.clone()],
            body,
            ..Default::default()
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    pub fn with_summary(mut self, line: impl Into<String>) -> Self {
        self.summary.push(line.into());
        self
    }

    pub fn with_detail(mut self, line: impl Into<String>) -> Self {
        self.detail.push(line.into());
        self
    }

    pub fn usage(body: impl Into<String>) -> Self {
        Self::new(body).with_code("EUSAGE")
    }

    /// The single-line error banner npm prints above the formatted message,
    /// e.g. `npm ERR! code E404` and `npm ERR! 404 Not Found - GET ...`.
    pub fn banner(&self, prefix: &str) -> String {
        let mut out = String::new();
        if let Some(code) = &self.code {
            let _ = writeln!(out, "{prefix} code {code}");
        }
        for line in &self.summary {
            let _ = writeln!(out, "{prefix} {line}");
        }
        out
    }
}

impl Default for NpmError {
    fn default() -> Self {
        Self {
            code: None,
            summary: Vec::new(),
            detail: Vec::new(),
            exit_code: None,
            suppress_error: false,
            body: String::new(),
        }
    }
}

impl From<std::io::Error> for NpmError {
    fn from(err: std::io::Error) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::NotFound => "ENOENT",
            std::io::ErrorKind::PermissionDenied => "EACCES",
            std::io::ErrorKind::AlreadyExists => "EEXIST",
            _ => "EIO",
        };
        Self::new(err.to_string()).with_code(code)
    }
}

pub type Result<T, E = NpmError> = std::result::Result<T, E>;

/// Unknown command, thrown from `Npm::cmd`.
pub fn unknown_command(command: &str) -> NpmError {
    NpmError::new(format!("Unknown command {command}")).with_code("EUNKNOWNCOMMAND")
}

/// Normalises a path-like argument so error messages and lookups agree,
/// mirroring how npm resolves everything to absolute paths up front.
pub fn resolve_path(base: &std::path::Path, p: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}
