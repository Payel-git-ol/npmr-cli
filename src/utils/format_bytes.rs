//! Port of `lib/utils/format-bytes.js`.

const COMPACT: bool = false;

const UNITS: [&str; 7] = ["B", "kB", "MB", "GB", "TB", "PB", "EB"];

/// Formats `bytes` as a human readable size using decimal units.
pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        return format!("{} B", bytes);
    }
    if COMPACT {
        format!("{:.0}{}", value, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn bytes_below_thousand() {
        assert_eq!(format_bytes(500), "500 B");
    }

    #[test]
    fn kilobytes() {
        assert_eq!(format_bytes(1500), "1.5 kB");
    }
}
