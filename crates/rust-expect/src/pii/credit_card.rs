//! Credit card detection and validation.

/// Credit card type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardType {
    /// Visa card (starts with 4).
    Visa,
    /// `MasterCard` (starts with 51-55).
    MasterCard,
    /// American Express (starts with 34 or 37).
    Amex,
    /// Discover card (starts with 6011).
    Discover,
    /// Unknown or unrecognized card type.
    Unknown,
}

impl CardType {
    /// Detect card type from number.
    #[must_use]
    pub fn detect(number: &str) -> Self {
        let digits: String = number.chars().filter(char::is_ascii_digit).collect();

        if digits.is_empty() {
            return Self::Unknown;
        }

        match digits.chars().next() {
            Some('4') => Self::Visa,
            Some('5') if digits.len() >= 2 => match digits.chars().nth(1) {
                Some('1'..='5') => Self::MasterCard,
                _ => Self::Unknown,
            },
            Some('3') if digits.len() >= 2 => match digits.chars().nth(1) {
                Some('4' | '7') => Self::Amex,
                _ => Self::Unknown,
            },
            Some('6') if digits.starts_with("6011") => Self::Discover,
            _ => Self::Unknown,
        }
    }
}

/// Validate a credit card number using Luhn algorithm.
#[must_use]
pub fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number
        .chars()
        .filter(char::is_ascii_digit)
        .filter_map(|c| c.to_digit(10))
        .collect();

    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }

    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();

    sum.is_multiple_of(10)
}

/// Mask a credit card number (show only last 4 digits).
#[must_use]
pub fn mask(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();

    if digits.len() >= 4 {
        let last4 = &digits[digits.len() - 4..];
        format!("****-****-****-{last4}")
    } else {
        "****-****-****-****".to_string()
    }
}

/// Format a card number with dashes.
#[must_use]
pub fn format(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();

    digits
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_type_detection() {
        assert_eq!(CardType::detect("4111111111111111"), CardType::Visa);
        assert_eq!(CardType::detect("5500000000000004"), CardType::MasterCard);
        assert_eq!(CardType::detect("340000000000009"), CardType::Amex);
    }

    #[test]
    fn luhn_valid() {
        assert!(luhn_check("4111111111111111"));
        assert!(luhn_check("5500000000000004"));
    }

    #[test]
    fn luhn_invalid() {
        assert!(!luhn_check("1234567890123456"));
    }

    #[test]
    fn mask_card() {
        assert_eq!(mask("4111111111111111"), "****-****-****-1111");
    }

    #[test]
    fn format_card() {
        assert_eq!(format("4111111111111111"), "4111-1111-1111-1111");
    }
}
