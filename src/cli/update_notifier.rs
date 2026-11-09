//! Port of `lib/cli/update-notifier.js`. Prints a banner when a newer npm is
//! available, at most once a day on prerelease trains and once a week
//! otherwise. The registry lookup itself is delegated (see TODO): there is no
//! network client in the crate yet.

use std::time::{Duration, SystemTime};

use semver::Version;

const DAILY: Duration = Duration::from_secs(60 * 60 * 24);
const WEEKLY: Duration = Duration::from_secs(DAILY.as_secs() * 7);

fn last_checked_file(cache: &std::path::Path) -> std::path::PathBuf {
    cache
        .parent()
        .unwrap_or(cache)
        .join("_update-notifier-last-checked")
}

/// Returns `None` when the notifier is disabled, running in CI, or has been
/// checked recently. TODO: fetch `npm@latest` from the registry and compare.
pub fn update_notifier(npm: &dyn crate::npm::Npm) -> Option<String> {
    if !npm.store().get_bool("update-notifier") {
        return None;
    }
    if std::env::var_os("CI").is_some() || std::env::var_os("CONTINUOUS_INTEGRATION").is_some() {
        return None;
    }
    if npm.global()
        && matches!(npm.command(), Some("install") | Some("update"))
        && npm
            .argv()
            .iter()
            .any(|a| a.starts_with("npm@") || a == "npm")
    {
        return None;
    }

    let current = Version::parse(npm.version()).ok()?;
    let duration = if current.pre.is_empty() {
        WEEKLY
    } else {
        DAILY
    };

    let marker = last_checked_file(&npm.store().cache());
    let threshold = SystemTime::now()
        .checked_sub(duration)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let recently_checked = std::fs::metadata(&marker)
        .and_then(|m| m.modified())
        .map(|mtime| mtime > threshold)
        .unwrap_or(false);
    if recently_checked {
        return None;
    }
    let _ = std::fs::write(&marker, "");
    // TODO: query the registry for the latest version and build the banner
    // with `update_banner(current, latest)`.
    None
}

/// Renders the "New version of npm available" banner.
pub fn update_banner(current: &Version, latest: &str, color: bool) -> String {
    let latest_version = Version::parse(latest).ok();
    let kind = match &latest_version {
        Some(l) if l.major != current.major => "major",
        Some(l) if l.minor != current.minor => "minor",
        Some(l) if l.patch != current.patch => "patch",
        _ => "prerelease",
    };
    let paint = |text: &str, color_code: &str| {
        if color {
            format!("\x1b[{color_code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    let kind_color = match kind {
        "major" => "31",
        "minor" => "33",
        _ => "36",
    };
    format!(
        "\nNew {} version of npm available! {} -> {}\nChangelog: {}\nTo update run: {}\n",
        paint(kind, kind_color),
        paint(&current.to_string(), kind_color),
        paint(latest, "34"),
        paint(
            &format!("https://github.com/npm/cli/releases/tag/v{latest}"),
            "34"
        ),
        paint(&format!("npm install -g npm@{latest}"), "4"),
    )
}
