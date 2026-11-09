//! Port of `lib/utils/read-user-info.js`: interactive prompts for username,
//! password, OTP and email.

use std::io::{BufRead, IsTerminal, Write};

use crate::utils::errors::NpmError;

pub const OTP_PROMPT: &str =
    "This command requires a one-time password (OTP) from your authenticator app.
Enter one below. You can also pass one on the command line by appending --otp=123456.
For more information, see:
https://docs.npmjs.com/getting-started/using-two-factor-authentication
Enter OTP: ";
pub const PASSWORD_PROMPT: &str = "npm password: ";
pub const USERNAME_PROMPT: &str = "npm username: ";
pub const EMAIL_PROMPT: &str = "email (this will be public): ";

/// Reads one line from stdin after printing `prompt` to stderr.
///
/// Echo is not suppressed; hidden password entry needs terminal control that
/// std does not expose.
pub fn read(prompt: &str) -> Result<String, NpmError> {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "{prompt}");
    let _ = err.flush();
    drop(err);

    let mut line = String::new();
    let read = std::io::stdin().lock().read_line(&mut line);
    match read {
        Ok(0) => Err(NpmError::new("User cancelled operation")),
        Ok(_) => Ok(line.trim_end_matches(['\r', '\n']).to_string()),
        Err(e) => Err(NpmError::new(format!("Could not read input: {e}"))),
    }
}

/// Reads and normalises a one-time password.
pub fn otp(default: &str) -> Result<String, NpmError> {
    let prompt = if default.is_empty() {
        OTP_PROMPT.to_string()
    } else {
        format!("{OTP_PROMPT}[{default}] ")
    };
    let mut value = read(&prompt)?;
    if value.is_empty() {
        value = default.to_string();
    }
    Ok(value.replace(char::is_whitespace, ""))
}

pub fn password(default: &str) -> Result<String, NpmError> {
    let prompt = if default.is_empty() {
        PASSWORD_PROMPT.to_string()
    } else {
        format!("{PASSWORD_PROMPT}[{default}] ")
    };
    let value = read(&prompt)?;
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value
    })
}

pub fn username(default: &str) -> Result<String, NpmError> {
    let prompt = if default.is_empty() {
        USERNAME_PROMPT.to_string()
    } else {
        format!("{USERNAME_PROMPT}[{default}] ")
    };
    let value = read(&prompt)?;
    Ok(if value.is_empty() {
        default.trim().to_string()
    } else {
        value.trim().to_string()
    })
}

pub fn email(default: &str) -> Result<String, NpmError> {
    let prompt = if default.is_empty() {
        EMAIL_PROMPT.to_string()
    } else {
        format!("{EMAIL_PROMPT}[{default}] ")
    };
    let value = read(&prompt)?;
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value
    })
}

pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}
