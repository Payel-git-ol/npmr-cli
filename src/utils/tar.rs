//! Port of `lib/utils/tar.js`: inspects a package tarball and logs its
//! contents (`npm pack`, `npm stage download`).

use std::collections::BTreeSet;

use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha512};

use crate::utils::format_bytes::format_bytes;
use crate::utils::tar_reader;

#[derive(Debug, Clone)]
pub struct TarFile {
    pub path: String,
    pub size: u64,
    pub mode: u32,
}

#[derive(Debug, Clone)]
pub struct TarContents {
    pub id: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub unpacked_size: u64,
    pub shasum: String,
    pub integrity: String,
    pub filename: String,
    pub files: Vec<TarFile>,
    pub entry_count: usize,
    pub bundled: Vec<String>,
}

fn normalize_name(name: &str) -> String {
    name.replace('@', "").replace('/', "-")
}

/// Reads every entry of an (uncompressed) tarball and summarises it.
pub fn get_contents(manifest: &Value, tarball: &[u8]) -> crate::utils::errors::Result<TarContents> {
    let name = manifest
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id = manifest
        .get("_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{name}@{version}"));

    let (files, entry_count, unpacked_size, bundled) = tar_reader::list_entries(tarball)?;

    let sha1 = Sha1::digest(tarball);
    let sha512 = Sha512::digest(tarball);

    Ok(TarContents {
        id,
        name: name.clone(),
        version: version.clone(),
        size: tarball.len() as u64,
        unpacked_size,
        shasum: hex::encode(sha1),
        integrity: format!("sha512-{}", base64(&sha512)),
        filename: format!("{}-{version}.tgz", normalize_name(&name)),
        files,
        entry_count,
        bundled,
    })
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Upper-case entries first, then everything else, both case-insensitively
/// ordered the way `localeCompare` with `numeric` does.
fn sort_files(files: &mut [TarFile]) {
    files.sort_by(|a, b| {
        let a_upper = is_upper(&a.path);
        let b_upper = is_upper(&b.path);
        b_upper
            .cmp(&a_upper)
            .then_with(|| natural_cmp(&a.path, &b.path))
    });
}

fn is_upper(path: &str) -> bool {
    path.chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a_low = a.to_lowercase();
    let b_low = b.to_lowercase();
    let mut ai = a_low.chars().peekable();
    let mut bi = b_low.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let mut na = String::new();
                let mut nb = String::new();
                while let Some(c) = ai.peek().copied().filter(|c| c.is_ascii_digit()) {
                    na.push(c);
                    ai.next();
                }
                while let Some(c) = bi.peek().copied().filter(|c| c.is_ascii_digit()) {
                    nb.push(c);
                    bi.next();
                }
                let va: u128 = na.parse().unwrap_or(0);
                let vb: u128 = nb.parse().unwrap_or(0);
                match va.cmp(&vb) {
                    std::cmp::Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(ca), Some(cb)) => {
                if ca != cb {
                    return ca.cmp(&cb);
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Logs the tarball summary the way `npm pack` does.
pub fn log_tar(contents: &TarContents, unicode: bool, json: bool, key: &str) {
    if json {
        let value = serde_json::json!({ key: tar_to_json(contents) });
        crate::output::set_meta(true);
        crate::output::standard(serde_json::to_string_pretty(&value).unwrap_or_default());
        return;
    }
    let notice = crate::utils::log::notice;
    notice("", &[&""]);
    let headline = if unicode {
        format!("📦 {}@{}", contents.name, contents.version)
    } else {
        format!("package: {}@{}", contents.name, contents.version)
    };
    notice("", &[&headline]);
    notice("Tarball Contents", &[&""]);
    let body = contents
        .files
        .iter()
        .filter(|f| !f.path.starts_with("node_modules/"))
        .map(|f| format!("{} {}", format_bytes(f.size), f.path))
        .collect::<Vec<_>>()
        .join("\n");
    if !body.is_empty() {
        notice("", &[&body]);
    }
    if !contents.bundled.is_empty() {
        notice("Bundled Dependencies", &[&""]);
        for name in &contents.bundled {
            notice("", &[name]);
        }
    }
    notice("Tarball Details", &[&""]);
    notice("", &[&format!("name: {}", contents.name)]);
    notice("", &[&format!("version: {}", contents.version)]);
    notice("", &[&format!("package size: {}", format_bytes(contents.size))]);
    notice("", &[&format!("unpacked size: {}", format_bytes(contents.unpacked_size))]);
    notice("", &[&format!("shasum: {}", contents.shasum)]);
    let integrity = &contents.integrity;
    let short = if integrity.len() > 80 {
        format!("{}[...]{}", &integrity[..20], &integrity[80..])
    } else {
        integrity.clone()
    };
    notice("", &[&format!("integrity: {short}")]);
    if !contents.bundled.is_empty() {
        notice("", &[&format!("bundled deps: {}", contents.bundled.len())]);
        notice(
            "",
            &[&format!("bundled files: {}", contents.entry_count - contents.files.len())],
        );
        notice("", &[&format!("own files: {}", contents.files.len())]);
    }
    notice("", &[&format!("total files: {}", contents.entry_count)]);
    notice("", &[&""]);
}

pub fn tar_to_json(contents: &TarContents) -> Value {
    serde_json::json!({
        "id": contents.id,
        "name": contents.name,
        "version": contents.version,
        "size": contents.size,
        "unpackedSize": contents.unpacked_size,
        "shasum": contents.shasum,
        "integrity": contents.integrity,
        "filename": contents.filename,
        "files": contents.files.iter().map(|f| serde_json::json!({
            "path": f.path,
            "size": f.size,
            "mode": f.mode,
        })).collect::<Vec<_>>(),
        "entryCount": contents.entry_count,
        "bundled": contents.bundled,
    })
}

/// Extracts `package/package.json` from a gzipped tarball.
pub fn read_manifest(tarball: &[u8]) -> crate::utils::errors::Result<Value> {
    let content = tar_reader::read_file(tarball, "package/package.json")?;
    serde_json::from_slice(&content)
        .map_err(|_| crate::utils::errors::NpmError::new("Could not read package.json from tarball"))
}

pub(crate) fn collect_bundled(path: &str, bundled: &mut BTreeSet<String>) {
    if let Some(rest) = path.strip_prefix("package/node_modules/") {
        let mut parts = rest.split('/');
        let first = parts.next().unwrap_or_default();
        let name = if first.starts_with('@') {
            match parts.next() {
                Some(second) => format!("{first}/{second}"),
                None => first.to_string(),
            }
        } else {
            first.to_string()
        };
        if !name.is_empty() {
            bundled.insert(name);
        }
    }
}

pub(crate) fn finish_files(mut files: Vec<TarFile>) -> Vec<TarFile> {
    sort_files(&mut files);
    files
}
