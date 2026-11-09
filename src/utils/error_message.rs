//! Port of `lib/utils/error-message.js`. Maps an [`NpmError`] onto the
//! human readable summary/detail blocks npm prints for known error codes.

use crate::utils::errors::NpmError;
use crate::utils::redact::redact;

/// A `(prefix, body)` pair as printed by npm (`npm ERR! <prefix> <body>`).
pub type Line = (String, String);

#[derive(Debug, Default, Clone)]
pub struct ErrorMessage {
    pub summary: Vec<Line>,
    pub detail: Vec<Line>,
    pub files: Vec<(String, String)>,
    pub json: Option<serde_json::Value>,
}

fn line(prefix: &str, body: impl Into<String>) -> Line {
    (prefix.to_string(), body.into())
}

/// Extracts the exit code carried by an error, if any.
pub fn get_exit_code_from_error(err: &NpmError) -> Option<i32> {
    err.exit_code
}

pub fn error_message(er: &mut NpmError, npm: &dyn crate::npm::Npm) -> ErrorMessage {
    let mut out = ErrorMessage::default();
    let message = redact(&er.body);

    match er.code.as_deref() {
        Some("ENOLOCK") => {
            let cmd = npm.command().unwrap_or_default().to_string();
            out.summary
                .push(line(&cmd, "This command requires an existing lockfile."));
            out.detail.push(line(
                &cmd,
                "Try creating one first with: npm i --package-lock-only",
            ));
            out.detail
                .push(line(&cmd, format!("Original error: {message}")));
        }
        Some("ENOAUDIT") => out.summary.push(line("audit", message)),
        Some("ECONNREFUSED") => {
            out.summary.push(line("", message));
            out.detail.push(line(
                "",
                "If you are behind a proxy, please make sure that the 'proxy' config is set properly.  See: 'npm help config'",
            ));
        }
        Some("EACCES") | Some("EPERM") => {
            out.summary.push(line("", message));
            let hint = if cfg!(windows) {
                "It's possible that the file was already in use (by a text editor or antivirus), or that you lack permissions to access it."
            } else {
                "It is likely you do not have the permissions to access this file as the current user"
            };
            out.detail.push(line(
                "",
                format!(
                    "The operation was rejected by your operating system.\n{hint}\n\nIf you believe this might be a permissions issue, please double-check the permissions of the file and its containing directories, or try running the command again as root/Administrator."
                ),
            ));
        }
        Some("EALLOWGIT") | Some("EALLOWREMOTE") => {
            out.summary.push(line("", message));
            out.detail.push(line("", "Refusing to fetch this package"));
        }
        Some("ENOGIT") => {
            out.summary.push(line("", message));
            out.detail.push(line(
                "",
                "Failed using git.\nPlease check if you have git installed and in your PATH.",
            ));
        }
        Some("EJSONPARSE") => {
            out.summary.push(line("JSON.parse", message));
            out.detail.push(line(
                "JSON.parse",
                "Failed to parse JSON data.\nNote: package.json must be actual JSON, not just JavaScript.",
            ));
        }
        Some("EOTP") | Some("E401") => {
            if er.code.as_deref() == Some("EOTP") || message.contains("one-time pass") {
                out.summary.push(line(
                    "",
                    "This operation requires a one-time password from your authenticator.",
                ));
                out.detail.push(line(
                    "",
                    "You can provide a one-time password by passing --otp=<code> to the command you ran.\nIf you already provided a one-time password then it is likely that you either typoed it, or it timed out. Please try again.",
                ));
            } else {
                out.summary.push(line(
                    "",
                    "Unable to authenticate, your authentication token seems to be invalid.",
                ));
                out.detail.push(line(
                    "",
                    "To correct this please try logging in again with:\n  npm login",
                ));
            }
        }
        Some("E404") => {
            let without_code = message.trim_start_matches("404 ").to_string();
            out.summary.push(line("404", without_code));
            if let Some(pkgid) = er.summary.first() {
                out.detail.push(line("404", ""));
                out.detail.push(line(
                    "404",
                    format!(
                        "The requested resource '{}' could not be found or you do not have permission to access it.",
                        redact(pkgid)
                    ),
                ));
            }
            out.detail.push(line("404", ""));
            out.detail
                .push(line("404", "Note that you can also install from a"));
            out.detail
                .push(line("404", "tarball, folder, http url, or git url."));
        }
        Some("EPUBLISHCONFLICT") => {
            out.summary.push(line(
                "publish fail",
                "Cannot publish over existing version.",
            ));
            out.detail.push(line(
                "publish fail",
                "Update the 'version' field in package.json and try again.",
            ));
            out.detail.push(line("publish fail", ""));
            out.detail.push(line(
                "publish fail",
                "To automatically increment version numbers, see:",
            ));
            out.detail.push(line("publish fail", "  npm help version"));
        }
        Some("EISGIT") => {
            out.summary.push(line("git", message));
            out.detail.push(line(
                "git",
                "Refusing to remove it. Update manually, or move it out of the way first.",
            ));
        }
        Some("EBADDEVENGINES") => {
            out.summary.push(line("EBADDEVENGINES", message));
        }
        Some("EEXIST") => {
            out.summary.push(line("", message));
            out.detail.push(line(
                "",
                "Remove the existing file and try again, or run npm",
            ));
            out.detail
                .push(line("", "with --force to overwrite files recklessly."));
        }
        Some("ENEEDAUTH") => {
            out.summary.push(line("need auth", message));
            out.detail.push(line(
                "need auth",
                "You need to authorize this machine using `npm login`",
            ));
        }
        Some("ECONNRESET")
        | Some("ENOTFOUND")
        | Some("ETIMEDOUT")
        | Some("ERR_SOCKET_TIMEOUT")
        | Some("EAI_FAIL") => {
            out.summary.push(line("network", message));
            out.detail.push(line(
                "network",
                "This is a problem related to network connectivity.\nIn most cases you are behind a proxy or have bad network settings.\n\nIf you are behind a proxy, please make sure that the 'proxy' config is set properly.  See: 'npm help config'",
            ));
        }
        Some("ETARGET") => {
            out.summary.push(line("notarget", message));
            out.detail.push(line(
                "notarget",
                "In most cases you or one of your dependencies are requesting a package version that doesn't exist.",
            ));
        }
        Some("E403") => {
            out.summary.push(line("403", message));
            out.detail.push(line(
                "403",
                "In most cases, you or one of your dependencies are requesting a package version that is forbidden by your security policy, or on a server you do not have access to.",
            ));
        }
        Some("ENOSPC") => {
            out.summary.push(line("nospc", message));
            out.detail.push(line(
                "nospc",
                "There appears to be insufficient space on your system to finish.\nClear up some disk space and try again.",
            ));
        }
        Some("EROFS") => {
            out.summary.push(line("rofs", message));
            out.detail.push(line(
                "rofs",
                "Often virtualized file systems, or other file systems that don't support symlinks, give this error.",
            ));
        }
        Some("ENOENT") => {
            out.summary.push(line("enoent", message));
            out.detail.push(line(
                "enoent",
                "This is related to npm not being able to find a file.",
            ));
        }
        _ => {
            out.summary.push(line("", message));
        }
    }

    out
}

/// Adds the formatted message to the error and returns the message so it can
/// be shown to the user, mirroring `getError`.
pub fn get_error(er: &mut NpmError, npm: &dyn crate::npm::Npm) -> ErrorMessage {
    if er.code.as_deref() == Some("EUNKNOWNCOMMAND") {
        let command = er.summary.first().cloned().unwrap_or_default();
        let command_name = command
            .strip_prefix("Unknown command ")
            .unwrap_or(&command)
            .to_string();
        let suggestions = crate::utils::did_you_mean::did_you_mean(None, &command_name);
        er.suppress_error = true;
        return ErrorMessage {
            summary: vec![(
                String::new(),
                format!(
                    "Unknown command: \"{command_name}\"\n{suggestions}\nTo see a list of supported npm commands, run:\n  npm help"
                ),
            )],
            ..Default::default()
        };
    }
    error_message(er, npm)
}
