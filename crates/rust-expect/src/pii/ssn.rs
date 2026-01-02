//! SSN-specific detection and validation.

use regex::Regex;
use std::sync::LazyLock;

static SSN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{3})-(\d{2})-(\d{4})\b")
        .expect("SSN pattern is a valid regex")
});

/// Validate an SSN format.
#[must_use]
pub fn is_valid_format(ssn: &str) -> bool {
    SSN_PATTERN.is_match(ssn)
}

/// Check if an SSN is in a known invalid range.
#[must_use]
pub fn is_valid_range(ssn: &str) -> bool {
    if let Some(caps) = SSN_PATTERN.captures(ssn) {
        let area: u16 = caps[1].parse().unwrap_or(0);
        let group: u16 = caps[2].parse().unwrap_or(0);
        let serial: u16 = caps[3].parse().unwrap_or(0);

        // Invalid area numbers
        if area == 0 || area == 666 || area >= 900 {
            return false;
        }

        // Invalid group or serial
        if group == 0 || serial == 0 {
            return false;
        }

        true
    } else {
        false
    }
}

/// Mask an SSN (show only last 4 digits).
#[must_use]
pub fn mask(ssn: &str) -> String {
    if let Some(caps) = SSN_PATTERN.captures(ssn) {
        format!("XXX-XX-{}", &caps[3])
    } else {
        "XXX-XX-XXXX".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_format() {
        assert!(is_valid_format("123-45-6789"));
        assert!(!is_valid_format("123456789"));
    }

    #[test]
    fn valid_range() {
        assert!(is_valid_range("123-45-6789"));
        assert!(!is_valid_range("000-45-6789"));
        assert!(!is_valid_range("666-45-6789"));
        assert!(!is_valid_range("900-45-6789"));
    }

    #[test]
    fn mask_ssn() {
        assert_eq!(mask("123-45-6789"), "XXX-XX-6789");
    }
}
