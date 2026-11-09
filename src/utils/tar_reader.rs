//! Minimal ustar reader/writer used by the pack and stage commands.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::Read;

use crate::utils::errors::{NpmError, Result};
use crate::utils::tar::{TarFile, collect_bundled};

const BLOCK: usize = 512;

fn parse_octal(field: &[u8]) -> u64 {
    let mut value: u64 = 0;
    for &b in field {
        if b == 0 || b == b' ' {
            continue;
        }
        let digit = b.wrapping_sub(b'0');
        if digit > 9 {
            continue;
        }
        value = value.wrapping_mul(8).wrapping_add(u64::from(digit));
    }
    value
}

fn split_name(header: &[u8]) -> (Option<&str>, &str) {
    let name = cstr_str(&header[0..100]);
    let prefix = cstr_str(&header[345..500]);
    if prefix.is_empty() {
        (None, name)
    } else {
        (Some(prefix), name)
    }
}

fn cstr_str(field: &[u8]) -> &str {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    std::str::from_utf8(&field[..end]).unwrap_or_default()
}

fn joined_name<'a>(prefix: Option<&str>, name: &'a str, buf: &'a mut String) -> &'a str {
    match prefix {
        None => name,
        Some(prefix) => {
            buf.clear();
            buf.push_str(prefix);
            buf.push('/');
            buf.push_str(name);
            buf.as_str()
        }
    }
}

fn is_zero_block(block: &[u8]) -> bool {
    block.iter().all(|&b| b == 0)
}

fn decompress(data: &[u8], mut out: Vec<u8>) -> Result<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    decoder
        .read_to_end(&mut out)
        .map_err(|e| NpmError::new(format!("Failed to decompress tarball: {e}")))?;
    Ok(out)
}

/// Decompresses (if needed) and walks a tarball. `visit` receives every
/// regular file; returning `false` stops the walk early.
fn walk(tarball: &[u8], mut visit: impl FnMut(&str, &[u8], u64, u32) -> bool) -> Result<()> {
    let raw = if tarball.starts_with(&[0x1f, 0x8b]) {
        // Uncompressed archives are roughly 3-4x the gzipped size; guessing
        // avoids repeated growth for the common case.
        Cow::Owned(decompress(
            tarball,
            Vec::with_capacity(tarball.len().saturating_mul(3)),
        )?)
    } else {
        Cow::Borrowed(tarball)
    };

    let mut path_buf = String::new();
    let mut offset = 0;
    while offset + BLOCK <= raw.len() {
        let header = &raw[offset..offset + BLOCK];
        offset += BLOCK;
        if is_zero_block(header) {
            break;
        }
        let (prefix, name) = split_name(header);
        let size = parse_octal(&header[124..136]);
        let mode = parse_octal(&header[100..108]) as u32;
        let typeflag = header[156];
        let data_end = (offset as u64 + size).min(raw.len() as u64) as usize;
        let data = &raw[offset..data_end];
        offset += (size as usize).div_ceil(BLOCK) * BLOCK;

        // Skip links, directories and device nodes; keep regular files.
        if (typeflag == b'0' || typeflag == 0)
            && !visit(joined_name(prefix, name, &mut path_buf), data, size, mode)
        {
            break;
        }
    }
    Ok(())
}

/// Decompresses (if needed) and walks a tarball, returning its entries.
pub struct Entry {
    pub path: String,
    pub data: Vec<u8>,
    pub size: u64,
    pub mode: u32,
}

pub fn entries(tarball: &[u8]) -> Result<Vec<Entry>> {
    let mut result = Vec::new();
    walk(tarball, |path, data, size, mode| {
        result.push(Entry {
            path: path.to_string(),
            data: data.to_vec(),
            size,
            mode,
        });
        true
    })?;
    Ok(result)
}

/// Returns `(files sorted, entry count, total size, bundled names)`.
pub fn list_entries(tarball: &[u8]) -> Result<(Vec<TarFile>, usize, u64, Vec<String>)> {
    let mut bundled = BTreeSet::new();
    let mut files = Vec::new();
    let mut total = 0;

    walk(tarball, |path, data, size, mode| {
        collect_bundled(path, &mut bundled);
        let stripped = path.strip_prefix("package/").unwrap_or(path);
        total += data.len() as u64;
        files.push(TarFile {
            path: stripped.to_string(),
            size,
            mode,
        });
        true
    })?;

    let count = files.len();
    Ok((
        super::tar::finish_files(files),
        count,
        total,
        bundled.into_iter().collect(),
    ))
}

/// Reads one member of a (possibly gzipped) tarball, stopping at the first
/// match instead of materialising every entry.
pub fn read_file(tarball: &[u8], path: &str) -> Result<Vec<u8>> {
    let mut found = None;
    walk(tarball, |entry_path, data, _, _| {
        if entry_path == path {
            found = Some(data.to_vec());
            false
        } else {
            true
        }
    })?;
    found.ok_or_else(|| NpmError::new(format!("{path} not found in tarball")))
}

fn write_octal(out: &mut [u8], value: u64) {
    let text = format!("{:o}", value);
    let len = out.len();
    let pad = len.saturating_sub(text.len() + 1);
    for b in out.iter_mut() {
        *b = b'0';
    }
    out[pad..pad + text.len()].copy_from_slice(text.as_bytes());
    *out.last_mut().unwrap() = 0;
}

/// Writes a ustar archive from `(path, data)` pairs.
pub fn write_tar(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (path, data) in files {
        let mut header = [0u8; BLOCK];
        let bytes = path.as_bytes();
        let split = bytes.len().min(100);
        header[..split].copy_from_slice(&bytes[..split]);
        write_octal(&mut header[100..108], 0o644);
        write_octal(&mut header[108..116], 0);
        write_octal(&mut header[116..124], 0);
        write_octal(&mut header[124..136], data.len() as u64);
        write_octal(&mut header[136..148], 0);
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let sum: u64 = header.iter().map(|&b| b as u64).sum();
        let sum_text = format!("{:06o}\0 ", sum);
        header[148..156].copy_from_slice(sum_text.as_bytes());

        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let padding = data.len().div_ceil(BLOCK) * BLOCK - data.len();
        out.extend(std::iter::repeat_n(0u8, padding));
    }
    out.extend(std::iter::repeat_n(0u8, BLOCK * 2));
    out
}
