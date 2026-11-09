//! Port of `lib/commands/outdated.js`. Building the list of outdated
//! packages needs the arborist tree and packuments; the output formatting
//! (json/parseable/pretty) is ported against a plain row list.

use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::display::{paint, Color};
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("outdated", "Check for outdated packages")
    .with_params(&[
        "all",
        "json",
        "long",
        "parseable",
        "global",
        "workspace",
        "before",
        "min-release-age",
        "min-release-age-exclude",
    ])
    .with_usage(&["[<package-spec> ...]"])
    .arborist();

/// A single outdated package row.
#[derive(Debug, Clone)]
pub struct OutdatedRow {
    pub name: String,
    pub path: String,
    pub dependency_type: &'static str,
    pub current: Option<String>,
    pub location: Option<String>,
    pub wanted: String,
    pub latest: String,
    pub workspace_dependent: Option<String>,
    pub depended_by_location: String,
    pub dependent: String,
    pub homepage: Option<String>,
}

impl OutdatedRow {
    /// Sorts rows alphabetically by name, then by dependent.
    pub fn sort(rows: &mut [OutdatedRow]) {
        rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.dependent.cmp(&b.dependent)));
    }

    fn type_label(&self) -> &'static str {
        match self.dependency_type {
            "optionalDependencies" => "optionalDependencies",
            "peerDependencies" => "peerDependencies",
            "devDependencies" => "devDependencies",
            _ => "dependencies",
        }
    }
}

/// `--parseable` output: `<path>:<name@wanted>:<name@current>:<name@latest>:<dependent>`.
pub fn parseable(rows: &[OutdatedRow], long: bool) -> String {
    rows.iter()
        .map(|row| {
            let current = match &row.current {
                Some(current) => format!("{}@{current}", row.name),
                None => "MISSING".to_string(),
            };
            let mut cols = vec![
                row.path.clone(),
                format!("{}@{}", row.name, row.wanted),
                current,
                format!("{}@{}", row.name, row.latest),
                row.dependent.clone(),
            ];
            if long {
                cols.push(row.type_label().to_string());
                cols.push(row.homepage.clone().unwrap_or_default());
                cols.push(row.depended_by_location.clone());
            }
            cols.join(":")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `--json` output: an object keyed by package name; duplicate names become
/// arrays, exactly as upstream does.
pub fn as_json(rows: &[OutdatedRow], long: bool) -> Value {
    let mut acc: Map<String, Value> = Map::new();
    for row in rows {
        let mut dep = Map::new();
        dep.insert(
            "current".into(),
            row.current.clone().map_or(Value::Null, Value::String),
        );
        dep.insert("wanted".into(), Value::String(row.wanted.clone()));
        dep.insert("latest".into(), Value::String(row.latest.clone()));
        dep.insert("dependent".into(), Value::String(row.dependent.clone()));
        dep.insert("location".into(), Value::String(row.path.clone()));
        if long {
            dep.insert("type".into(), Value::String(row.type_label().to_string()));
            dep.insert(
                "homepage".into(),
                row.homepage.clone().map_or(Value::Null, Value::String),
            );
            dep.insert(
                "dependedByLocation".into(),
                Value::String(row.depended_by_location.clone()),
            );
        }
        match acc.get(&row.name) {
            Some(Value::Array(existing)) => {
                let mut items = existing.clone();
                items.push(Value::Object(dep));
                acc.insert(row.name.clone(), Value::Array(items));
            }
            Some(existing) => {
                acc.insert(
                    row.name.clone(),
                    Value::Array(vec![existing.clone(), Value::Object(dep)]),
                );
            }
            None => {
                acc.insert(row.name.clone(), Value::Object(dep));
            }
        }
    }
    Value::Object(acc)
}

/// Renders the padded table npm prints by default.
pub fn pretty(rows: &[OutdatedRow], long: bool, color: bool) -> String {
    let mut header = vec![
        "Package".to_string(),
        "Current".to_string(),
        "Wanted".to_string(),
        "Latest".to_string(),
        "Location".to_string(),
        "Depended by".to_string(),
    ];
    if long {
        header.extend(
            ["Package Type", "Homepage", "Depended By Location"]
                .iter()
                .map(|s| s.to_string()),
        );
    }
    let mut table: Vec<Vec<String>> = vec![header];
    for row in rows {
        let name = if row.current.as_deref() == Some(row.wanted.as_str()) {
            paint(&row.name, Color::Yellow, color)
        } else {
            paint(&row.name, Color::Red, color)
        };
        let mut line = vec![
            name,
            row.current.clone().unwrap_or_else(|| "MISSING".into()),
            paint(&row.wanted, Color::Cyan, color),
            paint(&row.latest, Color::Blue, color),
            row.location.clone().unwrap_or_else(|| "-".into()),
            match &row.workspace_dependent {
                Some(dependent) => paint(dependent, Color::Blue, color),
                None => row.dependent.clone(),
            },
        ];
        if long {
            line.push(row.type_label().to_string());
            line.push(paint(row.homepage.as_deref().unwrap_or(""), Color::Blue, color));
            line.push(row.depended_by_location.clone());
        }
        table.push(line);
    }

    let right_align = [1usize, 2, 3];
    let mut widths = vec![0usize; table[0].len()];
    for line in &table {
        for (i, cell) in line.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for line in &table {
        let mut cells = Vec::new();
        for (i, cell) in line.iter().enumerate() {
            let pad = widths[i].saturating_sub(cell.chars().count());
            if right_align.contains(&i) {
                cells.push(format!("{}{cell}", " ".repeat(pad)));
            } else {
                cells.push(format!("{cell}{}", " ".repeat(pad)));
            }
        }
        while cells.last().is_some_and(|c| c.trim().is_empty()) {
            cells.pop();
        }
        let _ = writeln!(out, "{}", cells.join("  ").trim_end());
    }
    out.trim_end().to_string()
}

pub struct Outdated;

impl Outdated {
    pub fn new() -> Self {
        Self
    }

    /// Prints the rows in the configured format and sets the exit code.
    pub fn report(&self, npm: &dyn Npm, rows: &[OutdatedRow]) -> Result<()> {
        let long = npm.store().get_bool("long");
        let mut rows = rows.to_vec();
        OutdatedRow::sort(&mut rows);
        if !rows.is_empty() {
            crate::output::set_exit_code(1);
        }
        if npm.json() {
            let json = serde_json::to_string_pretty(&as_json(&rows, long))?;
            crate::output::standard(json);
            return Ok(());
        }
        let text = if npm.store().get_bool("parseable") {
            parseable(&rows, long)
        } else {
            pretty(&rows, long, npm.color())
        };
        if !text.is_empty() {
            crate::output::standard(text);
        }
        Ok(())
    }
}

impl Default for Outdated {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Outdated {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, _npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let _specs = args;
        Err(NpmError::new(
            "Cannot check for outdated packages: loading the install tree is not implemented yet",
        )
        .with_code("ENOTSUP"))
    }
}
