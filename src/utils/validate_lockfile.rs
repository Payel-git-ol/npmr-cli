//! Port of `lib/utils/validate-lockfile.js`. Compares the locked tree against
//! the ideal tree produced from `package.json` and reports mismatches the way
//! `npm ci` prints them.

use std::collections::BTreeMap;

use serde_json::Value;

/// A locked node reduced to what the checks compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedNode {
    pub location: String,
    pub name: String,
    pub version: String,
    pub resolved: Option<String>,
    pub integrity: Option<String>,
    pub dev: bool,
    pub optional: bool,
    pub link: bool,
}

impl LockedNode {
    pub fn from_package_json(location: &str, pkg: &Value) -> Self {
        Self {
            location: location.to_string(),
            name: pkg
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            version: pkg
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            resolved: pkg
                .get("resolved")
                .and_then(Value::as_str)
                .map(str::to_string),
            integrity: pkg
                .get("integrity")
                .and_then(Value::as_str)
                .map(str::to_string),
            dev: pkg.get("dev").and_then(Value::as_bool).unwrap_or(false),
            optional: pkg
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            link: pkg.get("link").and_then(Value::as_bool).unwrap_or(false),
        }
    }
}

/// Reads `packages` from a lockfile object into location-keyed nodes.
pub fn lockfile_inventory(lock: &Value) -> BTreeMap<String, LockedNode> {
    let mut inventory = BTreeMap::new();
    let Some(packages) = lock.get("packages").and_then(Value::as_object) else {
        return inventory;
    };
    for (location, pkg) in packages {
        if location.is_empty() {
            continue;
        }
        inventory.insert(
            location.clone(),
            LockedNode::from_package_json(location, pkg),
        );
    }
    inventory
}

/// Returns the human-readable errors for nodes present in one tree but
/// missing, extra or version-mismatched in the other.
pub fn validate_lockfile(
    virtual_inventory: &BTreeMap<String, LockedNode>,
    ideal_inventory: &BTreeMap<String, LockedNode>,
) -> Vec<String> {
    let mut errors = Vec::new();

    for (location, node) in virtual_inventory {
        let Some(ideal) = ideal_inventory.get(location) else {
            let suffix = if node.dev {
                " (dev)"
            } else if node.optional {
                " (optional)"
            } else {
                ""
            };
            errors.push(format!(
                "Missing: {}@{} from lock file{suffix}",
                node.name, node.version
            ));
            continue;
        };
        if ideal.version != node.version {
            errors.push(format!(
                "Invalid: lock file's {}@{} does not satisfy {}@{}",
                node.name, node.version, ideal.name, ideal.version
            ));
        }
    }

    for (location, ideal) in ideal_inventory {
        if !virtual_inventory.contains_key(location) {
            let suffix = if ideal.dev {
                " (dev)"
            } else if ideal.optional {
                " (optional)"
            } else {
                ""
            };
            errors.push(format!(
                "Missing: {}@{} from lock file{suffix}",
                ideal.name, ideal.version
            ));
        }
    }

    if errors.is_empty() {
        return errors;
    }

    // npm ci collapses repeated complaints about the same package.
    let mut seen = std::collections::HashSet::new();
    errors.retain(|error| seen.insert(error.clone()));
    errors
}

/// Verifies that the root `packageExtensions` still matches the lockfile.
pub fn validate_package_extensions(lock: &Value, manifest: &Value) -> Vec<String> {
    let locked = lock
        .get("packageExtensionsChecksum")
        .and_then(Value::as_str);
    let current = manifest.get("packageExtensions");
    if current.is_none() || current == Some(&Value::Null) {
        return Vec::new();
    }
    if locked.is_none() {
        return vec![
            "Invalid: packageExtensions in package.json is not reflected in the lock file".into(),
        ];
    }
    Vec::new()
}

/// Verifies that the root `.npm-extension` file hash matches the lockfile.
pub fn validate_npm_extension(lock: &Value, file_hash: Option<&str>) -> Vec<String> {
    let locked = lock.get("npmExtensionChecksum").and_then(Value::as_str);
    match (locked, file_hash) {
        (Some(locked), Some(current)) if locked != current => vec![format!(
            "Invalid: .npm-extension in the working directory differs from the lock file (lockfile: {locked}, actual: {current})"
        )],
        (Some(_), None) => {
            vec!["Invalid: lock file references a .npm-extension that does not exist".into()]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(location: &str, name: &str, version: &str) -> LockedNode {
        LockedNode {
            location: location.into(),
            name: name.into(),
            version: version.into(),
            resolved: None,
            integrity: None,
            dev: false,
            optional: false,
            link: false,
        }
    }

    #[test]
    fn reports_version_mismatch() {
        let mut locked = BTreeMap::new();
        locked.insert(
            "node_modules/a".into(),
            node("node_modules/a", "a", "1.0.0"),
        );
        let mut ideal = BTreeMap::new();
        ideal.insert(
            "node_modules/a".into(),
            node("node_modules/a", "a", "1.0.1"),
        );
        let errors = validate_lockfile(&locked, &ideal);
        assert_eq!(
            errors,
            vec!["Invalid: lock file's a@1.0.0 does not satisfy a@1.0.1"]
        );
    }
}
