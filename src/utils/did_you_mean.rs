//! Port of `lib/utils/did-you-mean.js`.

use crate::utils::cmd_list::COMMANDS;

const RUN_SCRIPTS: [&str; 4] = ["stop", "start", "test", "restart"];

/// Levenshtein edit distance (two-row dynamic programming).
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

fn is_close(scmd: &str, cmd: &str) -> bool {
    let len = scmd.chars().count().max(cmd.chars().count()) as f64;
    (distance(scmd, cmd) as f64) < len * 0.4
}

/// Suggests close commands, package scripts and bins for an unknown command.
/// `pkg` is the parsed `package.json` of the local prefix, when present.
pub fn did_you_mean(pkg: Option<&serde_json::Value>, scmd: &str) -> String {
    let scripts = pkg
        .and_then(|p| p.get("scripts"))
        .and_then(|s| s.as_object());
    let bins = pkg.and_then(|p| p.get("bin")).and_then(|b| b.as_object());

    let mut best: Vec<(String, String)> = Vec::new();

    for cmd in COMMANDS {
        if is_close(scmd, cmd) && scmd != *cmd {
            let description = crate::commands::description_for(cmd).unwrap_or_default();
            best.push(((*cmd).to_string(), description));
        }
    }
    if let Some(scripts) = scripts {
        for name in scripts.keys() {
            if is_close(scmd, name) && !RUN_SCRIPTS.contains(&name.as_str()) {
                best.push((
                    format!("run {name}"),
                    format!("run the \"{name}\" package script"),
                ));
            }
        }
    }
    if let Some(bins) = bins {
        for name in bins.keys() {
            if is_close(scmd, name) {
                best.push((
                    format!("exec {name}"),
                    format!("run the \"{name}\" command from either this or a remote npm package"),
                ));
            }
        }
    }

    if best.is_empty() {
        return String::new();
    }

    let which = if best.len() == 1 {
        "this"
    } else {
        "one of these"
    };
    let listing = best
        .iter()
        .take(3)
        .map(|(msg, comment)| format!("  npm {msg} # {comment}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n\nDid you mean {which}?\n{listing}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_basic() {
        assert_eq!(distance("kitten", "sitting"), 3);
    }

    #[test]
    fn suggests_scripts_and_bins() {
        let pkg = serde_json::json!({
            "bin": { "npx": "exists" },
            "scripts": { "install": "exists", "posttest": "exists" }
        });
        let sttest = did_you_mean(Some(&pkg), "sttest");
        assert!(sttest.contains("npm run posttest"), "{sttest}");
        let npxx = did_you_mean(Some(&pkg), "npxx");
        assert!(npxx.contains("npm exec npx"), "{npxx}");
        assert_eq!(did_you_mean(Some(&pkg), "qwuijbo"), "");
    }

    #[test]
    fn missing_scripts_and_bins_still_suggests_commands() {
        let pkg = serde_json::json!({ "name": "missing-bin" });
        assert!(did_you_mean(Some(&pkg), "nistall").contains("npm install"));
        assert!(did_you_mean(None, "nistall").contains("npm install"));
    }

    #[test]
    fn suggests_typo() {
        let suggestion = did_you_mean(None, "isntall");
        assert!(suggestion.contains("npm install"));
    }
}
