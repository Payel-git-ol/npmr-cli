//! Port of `lib/utils/installed-shallow.js`: completion for installed package
//! names, one level deep.

use crate::npm::Npm;

fn names(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('@') {
            if let Ok(subs) = std::fs::read_dir(entry.path()) {
                for sub in subs.flatten() {
                    out.push(format!("{name}/{}", sub.file_name().to_string_lossy()));
                }
            }
        } else if name != ".bin" {
            out.push(name);
        }
    }
    out.sort();
    out
}

/// Returns local names and global names (suffixed with ` -g`) for completion.
pub fn installed_shallow(npm: &dyn Npm, remain_len: usize) -> Option<Vec<String>> {
    if remain_len > 3 {
        return None;
    }
    let global = npm.global();
    let mut out = Vec::new();
    if !global {
        out.extend(names(&npm.local_dir()));
    }
    for name in names(&npm.global_dir()) {
        out.push(if global { name } else { format!("{name} -g") });
    }
    Some(out)
}
