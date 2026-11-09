//! Port of `lib/utils/open-url.js`.

use std::io::{IsTerminal, Write};

use crate::utils::errors::{NpmError, Result};

fn assert_valid_url(raw: &str) -> Result<()> {
    match url::Url::parse(raw) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => Ok(()),
        _ => Err(NpmError::new(format!("Invalid URL: {raw}")).with_code("EINVALIDURL")),
    }
}

fn output_msg(npm: &dyn crate::npm::Npm, title: &str, url: &str) {
    if npm.json() {
        npm.display()
            .standard(&serde_json::json!({ "title": title, "url": url }).to_string());
    } else {
        npm.display().standard(&format!("{title}:\n{url}"));
    }
}

/// Opens `url` in the configured browser, printing it when that fails.
pub fn open_url(npm: &dyn crate::npm::Npm, url: &str, title: &str, is_file: bool) -> Result<()> {
    let encoded: String =
        percent_encoding::utf8_percent_encode(url, percent_encoding::NON_ALPHANUMERIC).to_string();
    let browser = npm.store().get("browser").cloned();
    if browser.as_ref().and_then(|b| b.as_bool()) == Some(false) {
        output_msg(npm, title, &encoded);
        return Ok(());
    }
    if !is_file {
        assert_valid_url(&encoded)?;
    }
    let opener = match browser.as_ref().and_then(|b| b.as_str()) {
        Some(command) if !command.is_empty() => command.to_string(),
        _ => default_opener(),
    };
    match std::process::Command::new(opener).arg(&encoded).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            output_msg(npm, title, &encoded);
            Ok(())
        }
        Err(_) => {
            output_msg(npm, title, &encoded);
            Ok(())
        }
    }
}

/// Prints the url and, in an interactive terminal, waits for ENTER before
/// opening it.
pub fn open_url_prompt(
    npm: &dyn crate::npm::Npm,
    url: &str,
    title: &str,
    prompt: &str,
) -> Result<()> {
    assert_valid_url(url)?;
    output_msg(npm, title, url);
    if npm.store().get("browser").and_then(|b| b.as_bool()) == Some(false)
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        return Ok(());
    }
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    open_url(
        npm,
        url,
        "Browser unavailable. Please open the URL manually",
        false,
    )
}

fn default_opener() -> String {
    if cfg!(target_os = "macos") {
        "open".into()
    } else if cfg!(windows) {
        "explorer".into()
    } else {
        "xdg-open".into()
    }
}
