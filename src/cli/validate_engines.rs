//! Port of `lib/cli/validate-engines.js`. npmr is a native binary, so the
//! Node.js version check only matters for reporting why `npm install` scripts
//! would fail; keep the message shape identical.

pub const SUPPORTED_NODE: &str = "^22.22.2 || ^24.15.0 || >=26.0.0";

pub fn unsupported_message(version: &str, node: &str) -> String {
    format!(
        "npm {version} does not support Node.js {node}. This version of npm supports the following node versions: `{SUPPORTED_NODE}`. You can find the latest version at https://nodejs.org/."
    )
}

pub fn broken_message(version: &str, node: &str) -> String {
    format!(
        "ERROR: npm {version} is known not to run on Node.js {node}.  This version of npm supports the following node versions: `{SUPPORTED_NODE}`. You can find the latest version at https://nodejs.org/."
    )
}

/// Detects an installed `node` and returns its version, if any.
pub fn installed_node_version() -> Option<String> {
    let output = std::process::Command::new("node")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_start_matches('v')
            .to_string(),
    )
}

pub fn node_version_supported(node: &str) -> bool {
    let Ok(version) = semver::Version::parse(node) else {
        return false;
    };
    let req = semver::VersionReq::parse(SUPPORTED_NODE).expect("valid engines range");
    req.matches(&version)
}
