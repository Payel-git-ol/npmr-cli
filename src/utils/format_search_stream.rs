//! Port of `lib/utils/format-search-stream.js`: turns search result objects
//! into the text or JSON lines `npm search` prints.

use serde_json::Value;

use crate::utils::display::{Color, paint};

/// The package fields the formatter reads.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub name: String,
    pub description: Option<String>,
    pub maintainers: Vec<String>,
    pub publisher: Option<String>,
    /// `YYYY-MM-DD`, already formatted.
    pub date: Option<String>,
    pub keywords: Vec<String>,
    pub version: Option<String>,
    pub raw: Value,
}

impl SearchResult {
    pub fn from_json(value: &Value) -> Self {
        let strings = |key: &str| -> Vec<String> {
            value
                .get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let keywords = match value.get("keywords") {
            Some(Value::Array(_)) => strings("keywords"),
            Some(Value::String(s)) => s
                .replace(',', " ")
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        };
        let maintainers = value
            .get("maintainers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|m| match m {
                        Value::String(s) => Some(s.clone()),
                        Value::Object(o) => o
                            .get("username")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            description: value
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
            maintainers,
            publisher: value
                .get("publisher")
                .and_then(|p| p.get("username"))
                .and_then(Value::as_str)
                .map(str::to_string),
            date: value
                .get("date")
                .and_then(Value::as_str)
                .map(str::to_string),
            keywords,
            version: value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string),
            raw: value.clone(),
        }
    }

    fn words(&self) -> String {
        let mut words = vec![self.name.clone()];
        words.extend(self.maintainers.iter().cloned());
        words.extend(self.keywords.iter().cloned());
        words.join(" ").to_lowercase()
    }
}

/// Options shared by the two output formats.
pub struct FormatOptions {
    /// Search terms, used for highlighting (not for `--searchinclude`).
    pub args: Vec<String>,
    pub exclude: Vec<String>,
    pub parseable: bool,
    pub color: bool,
}

/// Removes ANSI escape sequences, mirroring `stripVTControlCharacters`.
fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether a result survives the `--searchexclude` filter.
pub fn keep(result: &SearchResult, exclude: &[String]) -> bool {
    let words = result.words();
    !exclude.iter().filter(|p| !p.is_empty()).any(|pattern| {
        if let Some(rest) = pattern.strip_prefix('/') {
            let body = rest.strip_suffix('/').unwrap_or(rest);
            regex::Regex::new(body).is_ok_and(|re| re.is_match(&words))
        } else {
            words.contains(pattern.as_str())
        }
    })
}

/// Renders one result for the JSON stream, or `None` when filtered out.
pub fn json_line(result: &SearchResult, exclude: &[String]) -> Option<String> {
    keep(result, exclude).then(|| result.raw.to_string())
}

/// Renders one result for the text/parseable stream.
pub fn text_block(result: &SearchResult, opts: &FormatOptions) -> Option<String> {
    if !keep(result, &opts.exclude) {
        return None;
    }
    let mut args: Vec<String> = opts
        .args
        .iter()
        .map(|a| a.to_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    args.sort_by_key(|a| a.len());
    let cyan = |s: &str| paint(s, Color::Cyan, opts.color);
    let blue = |s: &str| paint(s, Color::Blue, opts.color);

    let name = strip(&result.name);
    let description = strip(result.description.as_deref().unwrap_or(""));
    let date = result.date.clone().unwrap_or_else(|| "prehistoric".into());
    let version = result.version.clone().unwrap_or_default();
    let authors = result.maintainers.join(" ");
    let keywords: Vec<String> = result.keywords.iter().map(|k| strip(k)).collect();

    if opts.parseable {
        let cols = [
            name.clone(),
            description.clone(),
            authors.clone(),
            date.clone(),
            version.clone(),
            keywords.join(" "),
        ];
        let line = cols
            .iter()
            .filter(|c| !c.is_empty())
            .map(|c| c.replace('\t', " "))
            .collect::<Vec<_>>()
            .join("\t");
        return Some(line);
    }

    let highlight = |input: &str| -> String {
        let mut output = input.to_string();
        for arg in &args {
            let mut from = 0;
            while let Some(idx) = output.to_lowercase()[from..].find(arg.as_str()) {
                let start = from + idx;
                let end = start + arg.len();
                let highlit = cyan(&output[start..end]);
                output = format!("{}{}{}", &output[..start], highlit, &output[end..]);
                from = start + highlit.len();
            }
        }
        output
    };

    let keyword_line = keywords
        .iter()
        .map(|k| if args.contains(k) { cyan(k) } else { k.clone() })
        .collect::<Vec<_>>()
        .join(" ");

    let description = highlight(&description);
    let name = if args.contains(&name) {
        cyan(&name)
    } else {
        blue(&highlight(&name))
    };

    let mut out = if description.is_empty() {
        format!("{name}\n")
    } else {
        format!("{name}\n{description}\n")
    };
    let publisher = match &result.publisher {
        Some(publisher) => blue(publisher),
        None => paint("???", Color::Yellow, opts.color),
    };
    out.push_str(&format!(
        "Version {} published {} by {publisher}\n",
        blue(&version),
        blue(&date)
    ));
    out.push_str(&format!("Maintainers: {authors}\n"));
    if !keyword_line.is_empty() {
        out.push_str(&format!("Keywords: {keyword_line}\n"));
    }
    out.push_str(&format!("{}\n", blue(&format!("https://npm.im/{name}"))));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SearchResult {
        SearchResult::from_json(&serde_json::json!({
            "name": "cowboy",
            "description": "a cowboy hat",
            "maintainers": [{"username": "hats"}],
            "keywords": ["hat", "saddle"],
            "version": "1.0.0",
            "date": "2020-01-02",
        }))
    }

    #[test]
    fn parseable_line_has_tabs() {
        let opts = FormatOptions {
            args: Vec::new(),
            exclude: Vec::new(),
            parseable: true,
            color: false,
        };
        let line = text_block(&sample(), &opts).expect("result is kept");
        assert!(line.starts_with("cowboy\ta cowboy hat\thats\t2020-01-02\t1.0.0\t"));
    }

    #[test]
    fn exclude_filters_by_substring() {
        assert!(!keep(&sample(), &["cow".into()]));
        assert!(keep(&sample(), &["horse".into()]));
    }
}
