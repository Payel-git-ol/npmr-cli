//! Port of `lib/utils/patch-diff.js`: produces a git-compatible unified diff
//! between two directories, used by `npm patch commit`.

use std::collections::BTreeSet;
use std::path::Path;

/// Directory names never walked.
const IGNORE: &[&str] = &["node_modules", ".git"];

fn list_files(root: &Path, sub: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root.join(sub)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if sub.is_empty() {
            name.clone()
        } else {
            format!("{sub}/{name}")
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            if !IGNORE.contains(&name.as_str()) {
                list_files(root, &rel, out);
            }
        } else if file_type.is_file() {
            out.push(rel);
        }
    }
}

fn read_maybe(file: &Path) -> Option<String> {
    std::fs::read_to_string(file).ok()
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
        .iter()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// Longest-common-subsequence diff producing the hunks `diff`'s
/// `createTwoFilesPatch` emits.
fn file_patch(file: &str, a: Option<&str>, b: Option<&str>) -> String {
    let old = split_lines(a.unwrap_or(""));
    let new = split_lines(b.unwrap_or(""));

    let mut lcs = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let (mut i, mut j) = (0usize, 0usize);
    let mut ops: Vec<(char, &str)> = Vec::new();
    while i < old.len() && j < new.len() {
        if old[i] == new[j] {
            ops.push((' ', old[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            ops.push(('-', old[i]));
            i += 1;
        } else {
            ops.push(('+', new[j]));
            j += 1;
        }
    }
    while i < old.len() {
        ops.push(('-', old[i]));
        i += 1;
    }
    while j < new.len() {
        ops.push(('+', new[j]));
        j += 1;
    }

    let context = 3usize;
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, (kind, _))| *kind != ' ')
        .map(|(idx, _)| idx)
        .collect();

    let mut header = format!("--- a/{file}\n+++ b/{file}\n");
    if changed.is_empty() {
        return String::new();
    }

    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for &idx in &changed {
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(ops.len());
        match hunks.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => hunks.push((start, end)),
        }
    }

    for (start, end) in hunks {
        let old_start = ops[..start].iter().filter(|(k, _)| *k != '+').count() + 1;
        let new_start = ops[..start].iter().filter(|(k, _)| *k != '-').count() + 1;
        let old_len = ops[start..end].iter().filter(|(k, _)| *k != '+').count();
        let new_len = ops[start..end].iter().filter(|(k, _)| *k != '-').count();
        header.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            if old_len == 0 {
                old_start.saturating_sub(1)
            } else {
                old_start
            },
            old_len,
            if new_len == 0 {
                new_start.saturating_sub(1)
            } else {
                new_start
            },
            new_len
        ));
        for (kind, line) in &ops[start..end] {
            header.push(*kind);
            header.push_str(line);
            header.push('\n');
        }
    }
    header
}

/// Result of diffing two package directories.
pub struct DirDiff {
    pub diff: String,
    pub package_json_changed: bool,
}

/// Diffs `original_dir` against `edited_dir`. The root `package.json` is
/// excluded and only flagged, since arborist resolves the pre-patch manifest.
pub fn diff_dirs(original_dir: &Path, edited_dir: &Path, ignore: &BTreeSet<String>) -> DirDiff {
    let mut orig_files = Vec::new();
    let mut edit_files = Vec::new();
    list_files(original_dir, "", &mut orig_files);
    list_files(edited_dir, "", &mut edit_files);

    let all: BTreeSet<String> = orig_files.into_iter().chain(edit_files).collect();

    let mut diff = String::new();
    let mut package_json_changed = false;
    for file in all {
        let native = file.replace('/', std::path::MAIN_SEPARATOR_STR);
        let a = read_maybe(&original_dir.join(&native));
        let b = read_maybe(&edited_dir.join(&native));
        if a == b {
            continue;
        }
        if file == "package.json" {
            package_json_changed = true;
            continue;
        }
        if ignore.contains(&file) {
            continue;
        }
        let mut patch = file_patch(&file, a.as_deref(), b.as_deref());
        if a.is_none() {
            patch = patch.replace(&format!("--- a/{file}\n"), "--- /dev/null\n");
        }
        if b.is_none() {
            patch = patch.replace(&format!("+++ b/{file}\n"), "+++ /dev/null\n");
        }
        diff.push_str(&patch);
    }
    DirDiff {
        diff,
        package_json_changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_a_changed_line() {
        let patch = file_patch("index.js", Some("a\nb\n"), Some("a\nc\n"));
        assert!(patch.starts_with("--- a/index.js\n+++ b/index.js\n@@ -1,2 +1,2 @@\n a\n-b\n+c\n"));
    }

    #[test]
    fn marks_new_files_against_dev_null() {
        let patch = file_patch("new.txt", None, Some("x\n"));
        assert!(patch.contains("@@ -0,0 +1,1 @@"));
    }
}
