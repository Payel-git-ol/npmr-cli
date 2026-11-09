//! Port of `lib/utils/cli-only-flag.js`.

/// Returns true when `key` was set on the command line.
pub fn cli_only_flag(store: &crate::store::Store, key: &str) -> bool {
    store.find(key) == Some(crate::store::ConfigSource::Cli)
}

/// Marks config relaxations that were explicitly requested on the CLI so
/// install commands can report which safety checks were skipped.
pub fn patch_relax_opts_store(
    store: &crate::store::Store,
    flags: &[&str],
) -> std::collections::BTreeMap<String, bool> {
    flags
        .iter()
        .map(|flag| ((*flag).to_string(), cli_only_flag(store, flag)))
        .collect()
}
