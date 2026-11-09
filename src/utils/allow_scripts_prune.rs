//! Classifier behind `npm install-scripts prune`, ported from
//! `allow-scripts-prune.js`.

use crate::utils::allow_scripts_writer::{
    Policy, RemovedEntry, classify_unused_entries, ScriptNode,
};

pub use crate::utils::allow_scripts_writer::RemovedEntry as Removed;

pub fn classify(
    existing: &Policy,
    nodes: &[&ScriptNode],
) -> (Policy, Vec<RemovedEntry>) {
    classify_unused_entries(existing, nodes)
}
