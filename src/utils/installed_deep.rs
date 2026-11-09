//! Port of `lib/utils/installed-deep.js`: completion for every installed
//! package name, local and global.

use crate::npm::Npm;
use crate::utils::installed_shallow::installed_shallow;

/// Walks `node_modules` recursively and returns sorted unique names.
pub fn installed_deep(npm: &dyn Npm) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    let global = npm.global();
    collect(&npm.global_dir(), &mut found);
    if !global {
        collect(&npm.local_dir(), &mut found);
        if let Some(local) = installed_shallow(npm, 0) {
            found.extend(local);
        }
    }
    found.into_iter().collect()
}

fn collect(dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if name.starts_with('@') {
            if let Ok(subs) = std::fs::read_dir(&path) {
                for sub in subs.flatten() {
                    let sub_path = sub.path();
                    out.insert(format!("{name}/{}", sub.file_name().to_string_lossy()));
                    collect(&sub_path.join("node_modules"), out);
                }
            }
        } else {
            out.insert(name);
            collect(&path.join("node_modules"), out);
        }
    }
}
