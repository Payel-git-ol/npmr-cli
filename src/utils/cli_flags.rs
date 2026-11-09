//! Port of `lib/utils/cli-only-flag.js`: some flags are honoured only when
//! passed on the command line.

use serde_json::Value;

use crate::store::{ConfigSource, Store};

pub const PATCH_RELAX_FLAGS: &[&str] = &["allow-unused-patches", "ignore-patch-failures"];

/// Returns the relax flags that were set on the CLI, ready to merge into the
/// arborist options.
pub fn patch_relax_opts(store: &Store) -> serde_json::Map<String, Value> {
    let mut opts = serde_json::Map::new();
    for flag in PATCH_RELAX_FLAGS {
        if store.find(flag) == Some(ConfigSource::Cli) {
            if let Some(value) = store.get(flag) {
                opts.insert((*flag).to_string(), value.clone());
            }
        }
    }
    opts
}
