//! Classifier behind `npm install-scripts prune`, ported from
//! `allow-scripts-prune.js`.

use crate::utils::allow_scripts_writer::{
    Policy, RemovedEntry, ScriptNode, classify_unused_entries,
};

pub use crate::utils::allow_scripts_writer::RemovedEntry as Removed;

pub fn classify(existing: &Policy, nodes: &[&ScriptNode]) -> (Policy, Vec<RemovedEntry>) {
    classify_unused_entries(existing, nodes)
}
