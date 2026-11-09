//! Port of `lib/utils/validate-uuid.js`.

use std::sync::OnceLock;

pub fn uuid_regex() -> &'static regex::Regex {
    static REGEX: OnceLock<regex::Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        regex::Regex::new(
            r"^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
        )
        .expect("valid uuid regex")
    })
}

/// Returns a `EBADUUID` error when `value` is not a canonical RFC 4122 UUID.
pub fn validate_uuid(value: &str, what: &str) -> crate::utils::errors::Result<()> {
    if uuid_regex().is_match(value) {
        return Ok(());
    }
    Err(crate::utils::errors::NpmError::new(format!(
        "Invalid {what}: {value} is not a valid UUID"
    ))
    .with_code("EBADUUID"))
}

#[cfg(test)]
mod tests {
    use super::validate_uuid;

    #[test]
    fn accepts_v4() {
        assert!(validate_uuid("f81d4fae-7dec-41d0-a765-00a0c91e6bf6", "id").is_ok());
    }

    #[test]
    fn rejects_short() {
        assert!(validate_uuid("not-a-uuid", "id").is_err());
    }
}
