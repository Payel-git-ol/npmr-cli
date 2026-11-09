//! Port of `lib/utils/format.js`: `util.format`-style string interpolation
//! with `%s/%d/%j/%%` placeholders, then redaction.

use crate::utils::redact::redact;

/// Formats `fmt` with the given arguments the way `util.format` does for the
/// placeholder subset npm actually uses.
pub fn format(fmt: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(fmt.len());
    let mut args = args.iter();
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('%') => {
                chars.next();
                out.push('%');
            }
            Some('s') => {
                chars.next();
                if let Some(arg) = args.next() {
                    out.push_str(arg);
                } else {
                    out.push_str("%s");
                }
            }
            Some('d') => {
                chars.next();
                if let Some(arg) = args.next() {
                    out.push_str(&arg.parse::<f64>().unwrap_or(f64::NAN).to_string());
                } else {
                    out.push_str("%d");
                }
            }
            Some('j') => {
                chars.next();
                if let Some(arg) = args.next() {
                    let json = serde_json::from_str::<serde_json::Value>(arg)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|_| "[Circular]".to_string());
                    out.push_str(&json);
                } else {
                    out.push_str("%j");
                }
            }
            _ => out.push('%'),
        }
    }
    redact(&out)
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn substitutes_strings() {
        assert_eq!(format("using npm@%s", &["1.0.0"]), "using npm@1.0.0");
    }

    #[test]
    fn keeps_unknown_percent() {
        assert_eq!(format("100%", &[]), "100%");
    }
}
