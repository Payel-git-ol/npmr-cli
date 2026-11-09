//! Port of `lib/commands/view.js`. Argument parsing, local package lookup and
//! the pretty output are ported; fetching the packument needs `pacote` and
//! currently reports `ENOTSUP`.

use serde_json::{Map, Value};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::display::{Color, paint};
use crate::utils::errors::{NpmError, Result};

pub static META: CommandMeta = CommandMeta::new("view", "View registry info")
    .with_params(&[
        "json",
        "workspace",
        "workspaces",
        "include-workspace-root",
    ])
    .with_usage(&["[<package-spec>] [<field>[.subfield]...]"])
    .workspaces();

/// The first argument and the fields requested from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewArgs {
    pub pkg: String,
    pub local: bool,
    pub rest: Vec<String>,
}

pub fn parse_args(args: &[String]) -> ViewArgs {
    let (pkg, rest) = match args.split_first() {
        Some((first, rest)) => (first.clone(), rest.to_vec()),
        None => (".".to_string(), Vec::new()),
    };
    let local = pkg == "." || pkg.starts_with(".@");
    ViewArgs { pkg, local, rest }
}

fn unparse_person(d: &Value) -> String {
    let name = d.get("name").and_then(Value::as_str).unwrap_or("");
    let mut out = name.to_string();
    if let Some(email) = d.get("email").and_then(Value::as_str) {
        out.push_str(&format!(" <{email}>"));
    }
    if let Some(url) = d.get("url").and_then(Value::as_str) {
        out.push_str(&format!(" ({url})"));
    }
    out
}
#[expect(dead_code, reason = "used once field projection output is rendered")]
fn cleanup(data: &Value) -> Value {
    match data {
        Value::Array(items) => Value::Array(items.iter().map(cleanup).collect()),
        Value::Object(map) => {
            let len = map.len();
            let has_name = map.contains_key("name");
            let simple = has_name
                && (len == 1
                    || (len == 3 && map.contains_key("email") && map.contains_key("url"))
                    || (len == 2 && (map.contains_key("email") || map.contains_key("url")))
                    || map.contains_key("trustedPublisher"));
            if simple {
                Value::String(unparse_person(data))
            } else {
                data.clone()
            }
        }
        other => other.clone(),
    }
}

fn dep_line(name: &str, dep: &Value, color: bool) -> String {
    format!("{}: {}", paint(name, Color::Blue, color), dep)
}

fn dist_tags(packu: &Map<String, Value>) -> Vec<(String, String)> {
    let Some(tags) = packu.get("dist-tags").and_then(Value::as_object) else {
        return Vec::new();
    };
    let time = packu.get("time").and_then(Value::as_object);
    let published = |tag: &str, version: &str| -> i64 {
        if tag == "latest" {
            return i64::MAX;
        }
        time.and_then(|t| t.get(version))
            .and_then(Value::as_str)
            .and_then(parse_iso_date)
            .unwrap_or(0)
    };
    let mut tags: Vec<(String, String)> = tags
        .iter()
        .map(|(tag, version)| {
            let version = version.as_str().unwrap_or_default().to_string();
            (tag.clone(), version)
        })
        .collect();
    tags.sort_by(|(a_tag, a_ver), (b_tag, b_ver)| {
        published(a_tag, a_ver).cmp(&published(b_tag, b_ver)).then(b_tag.cmp(a_tag))
    });
    tags.reverse();
    tags
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.sss]Z` into epoch seconds.
fn parse_iso_date(text: &str) -> Option<i64> {
    let date = chrono::DateTime::parse_from_rfc3339(text).ok()?;
    Some(date.timestamp())
}

fn relative_date(iso: &str) -> String {
    let Some(then) = parse_iso_date(iso) else {
        return iso.to_string();
    };
    let now = chrono::Utc::now().timestamp();
    let diff = now - then;
    let plural = |n: i64, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    match diff {
        d if d < 0 => "in the future".to_string(),
        d if d < 60 => "just now".to_string(),
        d if d < 3600 => format!("{} ago", plural(d / 60, "minute")),
        d if d < 86400 => format!("{} ago", plural(d / 3600, "hour")),
        d if d < 2_592_000 => format!("{} ago", plural(d / 86400, "day")),
        d if d < 31_536_000 => format!("{} ago", plural(d / 2_592_000, "month")),
        d => format!("{} ago", plural(d / 31_536_000, "year")),
    }
}

/// Renders the human-readable whole-packument view.
pub fn pretty_view(packu: &Map<String, Value>, manifest: &Map<String, Value>, color: bool) -> String {
    let text = |key: &str| manifest.get(key).and_then(Value::as_str);
    let deps: Vec<String> = manifest
        .get("dependencies")
        .and_then(Value::as_object)
        .map(|deps| {
            deps.iter()
                .map(|(name, dep)| dep_line(name, dep, color))
                .collect()
        })
        .unwrap_or_default();
    let tags = dist_tags(packu);
    let site = manifest.get("homepage").map(|homepage| match homepage {
        Value::Object(map) => map.get("url").map(Value::to_string).unwrap_or_default(),
        other => other.as_str().map(str::to_string).unwrap_or(other.to_string()),
    });
    let bins: Vec<&str> = manifest
        .get("bin")
        .and_then(Value::as_object)
        .map(|bin| bin.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let license = match manifest.get("license") {
        Some(Value::String(license)) => license.clone(),
        Some(Value::Object(map)) => map
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("Proprietary")
            .to_string(),
        _ => "Proprietary".to_string(),
    };
    let version_count = packu
        .get("versions")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    let name = text("name").unwrap_or_default();
    let version = text("version").unwrap_or_default();
    let mut res: Vec<String> = vec![String::new()];
    res.push(
        [
            paint(&format!("{name}@{version}"), Color::Cyan, color),
            if license.to_lowercase().trim() == "proprietary" {
                paint(&license, Color::Red, color)
            } else {
                paint(&license, Color::Green, color)
            },
            format!(
                "deps: {}",
                if deps.is_empty() {
                    paint("none", Color::Cyan, color)
                } else {
                    paint(&deps.len().to_string(), Color::Cyan, color)
                }
            ),
            format!("versions: {}", paint(&version_count.to_string(), Color::Cyan, color)),
        ]
        .join(" | "),
    );
    if let Some(description) = text("description") {
        if !description.is_empty() {
            res.push(description.to_string());
        }
    }
    if let Some(site) = site.filter(|site| !site.is_empty()) {
        res.push(paint(&site, Color::Blue, color));
    }
    if let Some(deprecated) = text("deprecated") {
        res.push(format!(
            "\n{}!! - {deprecated}",
            paint("DEPRECATED", Color::Red, color)
        ));
    }
    if let Some(keywords) = packu.get("keywords").and_then(Value::as_array) {
        if !keywords.is_empty() {
            let list: Vec<String> = keywords
                .iter()
                .map(|k| paint(k.as_str().unwrap_or_default(), Color::Cyan, color))
                .collect();
            res.push(format!("\nkeywords: {}", list.join(", ")));
        }
    }
    if !bins.is_empty() {
        res.push(format!("\nbin: {}", paint(&bins.join(", "), Color::Cyan, color)));
    }
    let dist = manifest.get("dist").and_then(Value::as_object);
    if let Some(dist) = dist {
        let field = |key: &str| dist.get(key).and_then(Value::as_str).unwrap_or_default();
        res.push("\ndist".to_string());
        res.push(format!(
            ".tarball: {}",
            paint(field("tarball"), Color::Blue, color)
        ));
        res.push(format!(
            ".shasum: {}",
            paint(field("shasum"), Color::Green, color)
        ));
        if dist.get("integrity").is_some() {
            res.push(format!(
                ".integrity: {}",
                paint(field("integrity"), Color::Green, color)
            ));
        }
        if let Some(size) = dist.get("unpackedSize").and_then(Value::as_u64) {
            res.push(format!(
                ".unpackedSize: {}",
                paint(&crate::utils::format_bytes::format_bytes(size), Color::Blue, color)
            ));
        }
    }
    if !deps.is_empty() {
        const MAX_DEPS: usize = 24;
        res.push("\ndependencies:".to_string());
        res.push(deps[..deps.len().min(MAX_DEPS)].join(", "));
        if deps.len() > MAX_DEPS {
            res.push(format!("(...and {} more.)", deps.len() - MAX_DEPS));
        }
    }
    if let Some(maintainers) = packu.get("maintainers").and_then(Value::as_array) {
        if !maintainers.is_empty() {
            res.push("\nmaintainers:".to_string());
            for user in maintainers {
                res.push(format!("- {}", unparse_person(user)));
            }
        }
    }
    res.push("\ndist-tags:".to_string());
    const MAX_TAGS: usize = 5;
    let rendered: Vec<String> = tags
        .iter()
        .map(|(tag, version)| format!("{}: {version}", paint(tag, Color::Blue, color)))
        .collect();
    res.push(rendered[..rendered.len().min(MAX_TAGS)].join("\n"));
    if rendered.len() > MAX_TAGS {
        res.push(format!("(...and {} more.)", rendered.len() - MAX_TAGS));
    }
    let publisher = manifest.get("_npmUser").map(unparse_person);
    let publish_time = packu
        .get("time")
        .and_then(Value::as_object)
        .and_then(|time| time.get(version as &str))
        .and_then(Value::as_str);
    if publisher.is_some() || packu.get("time").is_some() {
        let mut info = "published".to_string();
        if let Some(time) = publish_time {
            info.push_str(&format!(" {}", paint(&relative_date(time), Color::Cyan, color)));
        }
        if let Some(publisher) = publisher {
            info.push_str(&format!(" by {publisher}"));
        }
        res.push(String::new());
        res.push(info);
    }
    res.join("\n")
}

pub struct View;

impl View {
    pub fn new() -> Self {
        Self
    }

    fn resolve_local(&self, npm: &dyn Npm, pkg: &str) -> Result<String> {
        if npm.global() {
            return Err(NpmError::new("Cannot use view command in global mode."));
        }
        let path = npm.prefix().join("package.json");
        let manifest = read_json(&path)?;
        let Some(name) = manifest.get("name").and_then(Value::as_str) else {
            return Err(NpmError::new("Invalid package.json, no \"name\" field"));
        };
        Ok(format!("{name}{}", &pkg[1..]))
    }

    fn view_package(&self, name: &str, _fields: &[String]) -> Result<()> {
        crate::utils::log::verbose("view", &[&format!("viewing {name}")]);
        Err(NpmError::new(format!(
            "Cannot view {name}: fetching packuments is not implemented yet"
        ))
        .with_code("ENOTSUP"))
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

fn read_json(path: &std::path::Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| NpmError::new(format!("{}: {err}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|err| NpmError::new(format!("{}: {err}", path.display())))
}

impl Command for View {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let parsed = parse_args(args);
        let pkg = if parsed.local {
            self.resolve_local(&*npm, &parsed.pkg)?
        } else {
            parsed.pkg
        };
        self.view_package(&pkg, &parsed.rest)
    }

    fn exec_workspaces(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let parsed = parse_args(args);
        if !parsed.local {
            crate::utils::log::warn("view", &[&"Ignoring workspaces for specified package(s)"]);
            let mut flat = vec![parsed.pkg];
            flat.extend(parsed.rest);
            return self.exec(npm, &flat);
        }
        let workspaces = crate::utils::set_workspaces::set_workspaces(&*npm)?;
        let suffix = &parsed.pkg[1..];
        for name in workspaces.keys() {
            self.view_package(&format!("{name}{suffix}"), &parsed.rest)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_to_local_dot() {
        assert_eq!(
            parse_args(&args(&[])),
            ViewArgs { pkg: ".".into(), local: true, rest: vec![] }
        );
    }

    #[test]
    fn detects_local_and_registry_specs() {
        assert!(parse_args(&args(&[".@1.0.0"])).local);
        assert!(!parse_args(&args(&["foo@1"])).local);
        assert_eq!(
            parse_args(&args(&["foo", "version", "dependencies"])).rest,
            vec!["version".to_string(), "dependencies".to_string()]
        );
    }

    #[test]
    fn cleanup_unparses_simple_people() {
        let person = serde_json::json!({ "name": "a", "email": "a@b.c" });
        assert_eq!(cleanup(&person), Value::String("a <a@b.c>".into()));
        let object = serde_json::json!({ "name": "a", "other": 1 });
        assert_eq!(cleanup(&object), object);
    }

    #[test]
    fn pretty_view_puts_latest_tag_first() {
        let packu = serde_json::json!({
            "versions": [1, 2],
            "dist-tags": { "next": "2.0.0", "latest": "1.0.0" },
            "time": { "1.0.0": "2020-01-01T00:00:00.000Z", "2.0.0": "2021-01-01T00:00:00.000Z" },
        });
        let manifest = serde_json::json!({ "name": "x", "version": "1.0.0" });
        let out = pretty_view(
            packu.as_object().unwrap(),
            manifest.as_object().unwrap(),
            false,
        );
        let tags_at = out.find("dist-tags:").unwrap();
        let latest = out[tags_at..].find("latest").unwrap();
        let next = out[tags_at..].find("next").unwrap();
        assert!(latest < next);
        assert!(out.contains("versions: 2"));
    }
}
