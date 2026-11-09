//! Port of `lib/commands/help-search.js`: greps the bundled docs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub static META: CommandMeta = CommandMeta::new("help-search", "Search npm help documentation")
    .with_usage(&["<text>"])
    .with_params(&["long"]);

pub struct HelpSearch;

impl HelpSearch {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpSearch {
    fn default() -> Self {
        Self::new()
    }
}

struct Doc {
    cmd: String,
    lines: Vec<Option<String>>,
    hits: BTreeMap<String, usize>,
}

fn strip_front_matter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + 5..].trim();
        }
    }
    trimmed.trim()
}

fn docs_dir(npm: &dyn Npm) -> PathBuf {
    npm.npm_root().join("docs/content")
}

fn collect_docs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_docs(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

fn search(args: &[String], files: &[PathBuf]) -> Vec<Doc> {
    let lowered: Vec<String> = args.iter().map(|a| a.to_lowercase()).collect();
    let mut results = Vec::new();

    for file in files {
        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };
        let content = strip_front_matter(&content);
        let lower = content.to_lowercase();
        if !lowered.iter().any(|a| lower.contains(a)) {
            continue;
        }

        let raw_lines: Vec<&str> = content.split('\n').collect();
        let mut lines: Vec<Option<String>> = vec![None; raw_lines.len()];
        let mut i = 0;
        while i < raw_lines.len() {
            let line = raw_lines[i];
            let next_matches = raw_lines
                .get(i + 1)
                .is_some_and(|n| lowered.iter().any(|a| n.to_lowercase().contains(a)));
            if next_matches {
                i += 3;
                continue;
            }
            if lowered.iter().any(|a| line.to_lowercase().contains(a)) {
                i += 2;
                continue;
            }
            lines[i] = Some(line.to_string());
            i += 1;
        }

        let mut pruned: Vec<Option<String>> = Vec::new();
        for line in lines {
            if line.is_none() && pruned.last().is_some_and(Option::is_none) {
                continue;
            }
            pruned.push(line);
        }
        while pruned.first().is_some_and(Option::is_none) {
            pruned.remove(0);
        }
        while pruned.last().is_some_and(Option::is_none) {
            pruned.pop();
        }

        let mut hits = BTreeMap::new();
        for line in pruned.iter().flatten() {
            let lower = line.to_lowercase();
            for (arg, needle) in args.iter().zip(&lowered) {
                let count = lower.matches(needle.as_str()).count();
                if count > 0 {
                    *hits.entry(arg.clone()).or_insert(0) += count;
                }
            }
        }
        if hits.is_empty() {
            continue;
        }

        let cmd = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        results.push(Doc { cmd, lines: pruned, hits });
    }

    results.sort_by(|a, b| {
        let total = |d: &Doc| d.hits.values().sum::<usize>();
        total(b).cmp(&total(a)).then_with(|| a.cmd.cmp(&b.cmd))
    });
    results
}

fn format_results(args: &[String], results: &[Doc], long: bool) -> String {
    let cols = 80usize;
    let mut out = Vec::new();

    for res in results {
        let hits = res
            .hits
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" ");
        if !long {
            let left = res.cmd.clone();
            let pad = cols.saturating_sub(left.len() + hits.len() + 1).max(1);
            out.push(format!("{left}{}{hits}", " ".repeat(pad)));
            continue;
        }
        out.push(String::new());
        out.push(String::new());
        out.push(res.cmd.clone());
        out.push("-".repeat(cols.saturating_sub(1)));
        for (i, line) in res.lines.iter().enumerate() {
            if i > 3 {
                break;
            }
            if let Some(line) = line {
                out.push(highlight(args, line));
            }
        }
    }

    if !long {
        let header = format!(
            "Top hits for {}\n{}",
            args.iter()
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(" "),
            "—".repeat(cols.saturating_sub(1))
        );
        let footer = format!(
            "{}\n(run with -l or --long to see more context)",
            "—".repeat(cols.saturating_sub(1))
        );
        format!("{header}\n{}\n{footer}", out.join("\n"))
            .trim()
            .to_string()
    } else {
        out.join("\n").trim().to_string()
    }
}

/// Wraps occurrences of the search terms in blue when colour is enabled.
fn highlight(args: &[String], line: &str) -> String {
    if !crate::utils::log::color() {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for arg in args {
        let needle = arg.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut start = 0;
        while let Some(pos) = lower[start..].find(&needle) {
            let begin = start + pos;
            ranges.push((begin, begin + needle.len()));
            start = begin + needle.len();
        }
    }
    if ranges.is_empty() {
        return line.to_string();
    }
    ranges.sort();
    let mut out = String::new();
    let mut cursor = 0;
    for (start, end) in ranges {
        if start < cursor {
            continue;
        }
        out.push_str(&line[cursor..start]);
        out.push_str(&crate::utils::display::paint(
            &line[start..end],
            crate::utils::display::Color::Blue,
            true,
        ));
        cursor = end;
    }
    out.push_str(&line[cursor..]);
    out
}

impl Command for HelpSearch {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(self.meta().usage_error(""));
        }
        let mut files = Vec::new();
        collect_docs(&docs_dir(npm), &mut files);
        files.sort();

        let results = search(args, &files);
        let long = npm.store().get_bool("long");
        let formatted = format_results(args, &results, long);
        crate::output::set_meta(false);
        if formatted.trim().is_empty() {
            crate::output::standard(format!("No matches in help for: {}\n", args.join(" ")));
        } else {
            crate::output::standard(formatted);
        }
        Ok(())
    }
}
