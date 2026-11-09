//! Minimal ustar reader/writer used by the pack and stage commands.

use std::collections::BTreeSet;
use std::io::Read;

use crate::utils::errors::{NpmError, Result};
use crate::utils::tar::{TarFile, collect_bundled};

const BLOCK: usize = 512;

fn parse_octal(field: &[u8]) -> u64 {
    let text: String = field
        .iter()
        .copied()
        .take_while(|&b| b != 0 && b != b' ')
        .map(|b| b as char)
        .collect();
    u64::from_str_radix(text.trim(), 8).unwrap_or(0)
}

fn entry_name(header: &[u8]) -> String {
    let name = &header[0..100];
    let prefix = &header[345..500];
    let name = cstr(name);
    let prefix = cstr(prefix);
    if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    }
}

fn cstr(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

fn is_zero_block(block: &[u8]) -> bool {
    block.iter().all(|&b| b == 0)
}

fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| NpmError::new(format!("Failed to decompress tarball: {e}")))?;
    Ok(out)
}

/// Decompresses (if needed) and walks a tarball, returning its entries.
pub fn entries(tarball: &[u8]) -> Result<Vec<(String, Vec<u8>, u64, u32)>> {
    let raw = if tarball.starts_with(&[0x1f, 0x8b]) {
        decompress(tarball)?
    } else {
        tarball.to_vec()
    };

    let mut result = Vec::new();
    let mut offset = 0;
    while offset + BLOCK <= raw.len() {
        let header = &raw[offset..offset + BLOCK];
        offset += BLOCK;
        if is_zero_block(header) {
            break;
        }
        let name = entry_name(header);
        let size = parse_octal(&header[124..136]) as usize;
        let mode = parse_octal(&header[100..108]) as u32;
        let typeflag = header[156];
        let data_end = (offset + size).min(raw.len());
        let data = raw[offset..data_end].to_vec();
        offset += size.div_ceil(BLOCK) * BLOCK;

        // Skip links, directories and device nodes; keep regular files.
        if typeflag == b'0' || typeflag == 0 {
            result.push((name, data, size as u64, mode));
        }
    }
    Ok(result)
}

/// Returns `(files sorted, entry count, total size, bundled names)`.
pub fn list_entries(tarball: &[u8]) -> Result<(Vec<TarFile>, usize, u64, Vec<String>)> {
    let all = entries(tarball)?;
    let mut bundled = BTreeSet::new();
    let mut files = Vec::with_capacity(all.len());
    let mut total = 0;

    for (path, data, size, mode) in all {
        collect_bundled(&path, &mut bundled);
        let stripped = path.strip_prefix("package/").unwrap_or(&path);
        total += data.len() as u64;
        files.push(TarFile {
            path: stripped.to_string(),
            size,
            mode,
        });
    }
    let count = files.len();
    Ok((super::tar::finish_files(files), count, total, bundled.into_iter().collect()))
}

/// Reads one member of a (possibly gzipped) tarball.
pub fn read_file(tarball: &[u8], path: &str) -> Result<Vec<u8>> {
    entries(tarball)?
        .into_iter()
        .find(|(name, ..)| name == path)
        .map(|(.., data, _, _)| data)
        .ok_or_else(|| NpmError::new(format!("{path} not found in tarball")))
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
