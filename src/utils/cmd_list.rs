//! Port of `lib/utils/cmd-list.js`. Keeps the command list, aliases and the
//! `deref` resolver that maps user input (aliases, camelCase, abbreviated
//! commands) onto a canonical command name.

use std::collections::BTreeMap;

/// These correspond to modules in `src/commands`.
/// Please keep this list sorted alphabetically.
pub const COMMANDS: &[&str] = &[
    "access",
    "approve-scripts",
    "audit",
    "bugs",
    "cache",
    "ci",
    "completion",
    "config",
    "dedupe",
    "deny-scripts",
    "deprecate",
    "diff",
    "dist-tag",
    "docs",
    "doctor",
    "edit",
    "exec",
    "explain",
    "explore",
    "find-dupes",
    "fund",
    "get",
    "help",
    "help-search",
    "init",
    "install",
    "install-ci-test",
    "install-scripts",
    "install-test",
    "link",
    "ll",
    "login",
    "logout",
    "ls",
    "org",
    "outdated",
    "owner",
    "pack",
    "patch",
    "ping",
    "pkg",
    "prefix",
    "profile",
    "prune",
    "publish",
    "query",
    "rebuild",
    "repo",
    "restart",
    "root",
    "run",
    "sbom",
    "search",
    "set",
    "stage",
    "start",
    "stop",
    "team",
    "test",
    "token",
    "trust",
    "undeprecate",
    "uninstall",
    "unpublish",
    "update",
    "version",
    "view",
    "whoami",
];

/// Aliases must resolve to an entry in [`COMMANDS`].
pub fn aliases() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        // aliases
        ("author", "owner"),
        ("home", "docs"),
        ("issues", "bugs"),
        ("info", "view"),
        ("show", "view"),
        ("find", "search"),
        ("add", "install"),
        ("unlink", "uninstall"),
        ("remove", "uninstall"),
        ("rm", "uninstall"),
        ("r", "uninstall"),
        // short names for common things
        ("un", "uninstall"),
        ("rb", "rebuild"),
        ("list", "ls"),
        ("ln", "link"),
        ("create", "init"),
        ("i", "install"),
        ("it", "install-test"),
        ("cit", "install-ci-test"),
        ("u", "update"),
        ("up", "update"),
        ("c", "config"),
        ("s", "search"),
        ("se", "search"),
        ("tst", "test"),
        ("t", "test"),
        ("ddp", "dedupe"),
        ("v", "view"),
        ("run-script", "run"),
        ("clean-install", "ci"),
        ("clean-install-test", "install-ci-test"),
        ("x", "exec"),
        ("why", "explain"),
        ("la", "ll"),
        ("verison", "version"),
        ("ic", "ci"),
        // typos
        ("innit", "init"),
        // manually abbrev so that install-test doesn't make insta stop working
        ("in", "install"),
        ("ins", "install"),
        ("inst", "install"),
        ("insta", "install"),
        ("instal", "install"),
        ("isnt", "install"),
        ("isnta", "install"),
        ("isntal", "install"),
        ("isntall", "install"),
        ("install-clean", "ci"),
        ("isntall-clean", "ci"),
        ("hlep", "help"),
        ("dist-tags", "dist-tag"),
        ("upgrade", "update"),
        ("udpate", "update"),
        ("rum", "run"),
        ("sit", "install-ci-test"),
        ("urn", "run"),
        ("ogr", "org"),
    ])
}

/// Translates camelCase to kebab-case (`installTest` -> `install-test`).
fn camel_to_kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Computes unique-prefix abbreviations for all commands and aliases, the
/// Rust equivalent of the `abbrev` package.
fn abbrevs() -> BTreeMap<String, String> {
    let mut names: Vec<&str> = COMMANDS.to_vec();
    let alias_map = aliases();
    names.extend(alias_map.keys().copied());

    let mut map = BTreeMap::new();
    for name in &names {
        let chars: Vec<char> = name.chars().collect();
        for len in 1..=chars.len() {
            let prefix: String = chars[..len].iter().collect();
            match map.get_mut(&prefix) {
                None => {
                    map.insert(prefix, (*name).to_string());
                }
                Some(existing) if existing != name => {
                    *existing = String::new();
                }
                Some(_) => {}
            }
        }
    }
    map.retain(|_, v| !v.is_empty());
    map
}

/// Resolves user input to a canonical command name.
pub fn deref(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    let mut c = input.to_string();
    if c.chars().any(|ch| ch.is_ascii_uppercase()) {
        c = camel_to_kebab(&c);
    }
    if COMMANDS.contains(&c.as_str()) {
        return Some(c);
    }
    let alias_map = aliases();
    if let Some(target) = alias_map.get(c.as_str()) {
        return Some((*target).to_string());
    }
    let mut a = abbrevs().get(&c).cloned();
    while let Some(current) = a.clone() {
        match alias_map.get(current.as_str()) {
            Some(next) => a = Some((*next).to_string()),
            None => break,
        }
    }
    a
}

/// Every alias that resolves to `name`.
pub fn aliases_for(name: &str) -> Vec<&'static str> {
    aliases()
        .into_iter()
        .filter(|(_, target)| *target == name)
        .map(|(alias, _)| alias)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_alias() {
        assert_eq!(deref("install").as_deref(), Some("install"));
        assert_eq!(deref("i").as_deref(), Some("install"));
        assert_eq!(deref("installTest").as_deref(), Some("install-test"));
    }

    #[test]
    fn abbreviation_through_alias() {
        assert_eq!(deref("install-cl").as_deref(), Some("ci"));
    }

    #[test]
    fn unknown_is_none() {
        assert_eq!(deref("frobnicate"), None);
    }
}
