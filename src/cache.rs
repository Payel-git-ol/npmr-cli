//! In-memory caches backed by [`moka::sync::Cache`].
//!
//! Entries are keyed by path and validated against the file's modification
//! time, so commands that rewrite `package.json` or `.npmrc` (`init`, `version`,
//! `patch`, `config set`) never observe stale data.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::SystemTime;

use serde_json::Value;

use crate::utils::errors::NpmError;

/// Capacity of the parsed-manifest cache.
pub const MANIFEST_CAPACITY: u64 = 1024;
/// Capacity of the parsed-`.npmrc` cache.
pub const CONFIG_CAPACITY: u64 = 64;

/// Result of reading and parsing a file. The payload is kept behind an `Arc`
/// so lookups hand it out without copying the parsed tree.
///
/// Parse failures are cached too: re-reading a malformed file on every lookup
/// would only repeat the same work.
#[derive(Debug, Clone)]
pub struct CachedFile<T> {
    modified: Option<SystemTime>,
    content: Arc<std::result::Result<T, NpmError>>,
}

impl<T: Clone> CachedFile<T> {
    fn new(modified: Option<SystemTime>, content: std::result::Result<T, NpmError>) -> Self {
        Self {
            modified,
            content: Arc::new(content),
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

fn read_validated<T: Clone + Send + Sync + 'static>(
    cache: &moka::sync::Cache<PathBuf, CachedFile<T>>,
    path: &Path,
    parse: impl FnOnce(&str) -> std::result::Result<T, NpmError>,
) -> std::result::Result<T, NpmError> {
    let modified = mtime(path);
    if let Some(entry) = cache.get(path)
        && entry.modified == modified
        && modified.is_some()
    {
        return (*entry.content).clone();
    }

    let content = std::fs::read_to_string(path)
        .map_err(NpmError::from)
        .and_then(|text| parse(&text));
    cache.insert(
        path.to_path_buf(),
        CachedFile::new(modified, content.clone()),
    );
    content
}

static MANIFESTS: LazyLock<moka::sync::Cache<PathBuf, CachedFile<Value>>> = LazyLock::new(|| {
    moka::sync::Cache::builder()
        .max_capacity(MANIFEST_CAPACITY)
        .build()
});

/// One `(key, value)` pair of an ini-style config file.
pub type IniEntry = (String, String);

static CONFIGS: LazyLock<moka::sync::Cache<PathBuf, CachedFile<Vec<IniEntry>>>> =
    LazyLock::new(|| {
        moka::sync::Cache::builder()
            .max_capacity(CONFIG_CAPACITY)
            .build()
    });

/// Reads `path` as JSON, reusing the parsed manifest while its mtime is
/// unchanged.
pub fn read_json(path: &Path) -> crate::utils::errors::Result<Value> {
    read_validated(&MANIFESTS, path, |text| {
        serde_json::from_str(text).map_err(NpmError::from)
    })
}

/// Reads `path` as a `package.json` and requires it to be an object.
pub fn read_manifest(path: &Path) -> crate::utils::errors::Result<serde_json::Map<String, Value>> {
    match read_json(path)? {
        Value::Object(map) => Ok(map),
        _ => Err(NpmError::new("manifest is not a JSON object")),
    }
}

/// Reads and parses an ini-style config file into `(key, value)` pairs,
/// dropping blank lines and `#`/`;` comments.
pub fn read_ini(path: &Path) -> crate::utils::errors::Result<Vec<IniEntry>> {
    read_validated(&CONFIGS, path, |text| Ok(parse_ini(text)))
}

fn parse_ini(text: &str) -> Vec<IniEntry> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                return None;
            }
            let (key, value) = line.split_once(['=', ':'])?;
            Some((
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("npmr-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn rewrite(path: &Path, content: &str) {
        // Bump mtime past the filesystem's timestamp granularity.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn manifest_cache_notices_rewrites() {
        let path = temp_file("rewrite.json", r#"{"name":"a"}"#);
        assert_eq!(read_json(&path).unwrap()["name"], "a");
        rewrite(&path, r#"{"name":"b"}"#);
        assert_eq!(read_json(&path).unwrap()["name"], "b");
    }

    #[test]
    fn parse_errors_are_reported() {
        let path = temp_file("broken.json", "{");
        assert!(read_json(&path).is_err());
        assert!(read_json(&path).is_err());
    }

    #[test]
    fn ini_parser_skips_comments() {
        let path = temp_file(
            "config.npmrc",
            "# comment\n; other\nregistry = \"https://example.com/\"\n\nfetch-timeout=1000\n",
        );
        assert_eq!(
            read_ini(&path).unwrap(),
            vec![
                ("registry".to_string(), "https://example.com/".to_string()),
                ("fetch-timeout".to_string(), "1000".to_string()),
            ]
        );
    }

    #[test]
    fn missing_files_are_errors() {
        let path = std::env::temp_dir().join("npmr-cache-does-not-exist.json");
        let _ = std::fs::remove_file(&path);
        assert!(read_json(&path).is_err());
    }
}
