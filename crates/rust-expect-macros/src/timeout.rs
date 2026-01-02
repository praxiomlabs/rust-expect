//! Timeout duration parsing macro implementation.
//!
//! This module implements the `timeout!` macro for parsing human-readable
//! timeout specifications at compile time.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitInt, Result, Token};

/// A timeout specification.
pub struct TimeoutInput {
    /// The duration components.
    pub components: Vec<TimeoutComponent>,
}

/// A single timeout component (e.g., "5s" or "100ms").
pub struct TimeoutComponent {
    /// The numeric value.
    pub value: u64,
    /// The unit (s, ms, us, m, h).
    pub unit: TimeoutUnit,
}

/// Time units supported by the timeout macro.
#[derive(Clone, Copy)]
pub enum TimeoutUnit {
    /// Nanoseconds
    Nanoseconds,
    /// Microseconds
    Microseconds,
    /// Milliseconds
    Milliseconds,
    /// Seconds
    Seconds,
    /// Minutes
    Minutes,
    /// Hours
    Hours,
}

impl TimeoutUnit {
    /// Convert to nanoseconds multiplier.
    const fn to_nanos(self) -> u64 {
        match self {
            Self::Nanoseconds => 1,
            Self::Microseconds => 1_000,
            Self::Milliseconds => 1_000_000,
            Self::Seconds => 1_000_000_000,
            Self::Minutes => 60 * 1_000_000_000,
            Self::Hours => 60 * 60 * 1_000_000_000,
        }
    }
}

impl Parse for TimeoutComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        let value: LitInt = input.parse()?;
        let value_u64: u64 = value.base10_parse()?;

        let unit: Ident = input.parse()?;
        let unit = match unit.to_string().as_str() {
            "ns" | "nanos" | "nanoseconds" => TimeoutUnit::Nanoseconds,
            "us" | "micros" | "microseconds" => TimeoutUnit::Microseconds,
            "ms" | "millis" | "milliseconds" => TimeoutUnit::Milliseconds,
            "s" | "sec" | "secs" | "seconds" => TimeoutUnit::Seconds,
            "m" | "min" | "mins" | "minutes" => TimeoutUnit::Minutes,
            "h" | "hr" | "hrs" | "hours" => TimeoutUnit::Hours,
            other => {
                return Err(syn::Error::new(
                    unit.span(),
                    format!("unknown time unit: {other}"),
                ));
            }
        };

        Ok(Self {
            value: value_u64,
            unit,
        })
    }
}

impl Parse for TimeoutInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut components = Vec::new();

        // Parse first component
        components.push(input.parse()?);

        // Parse additional components separated by +
        while input.peek(Token![+]) {
            let _: Token![+] = input.parse()?;
            components.push(input.parse()?);
        }

        Ok(Self { components })
    }
}

/// Generate code for the timeout! macro.
pub fn expand(input: TimeoutInput) -> TokenStream {
    // Calculate total duration in nanoseconds at compile time
    let total_nanos: u64 = input
        .components
        .iter()
        .map(|c| c.value.saturating_mul(c.unit.to_nanos()))
        .fold(0u64, u64::saturating_add);

    let secs = total_nanos / 1_000_000_000;
    let nanos = (total_nanos % 1_000_000_000) as u32;

    quote! {
        std::time::Duration::new(#secs, #nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn parse_simple_timeout() {
        let input: TimeoutInput = parse_quote! {
            5 s
        };
        assert_eq!(input.components.len(), 1);
        assert_eq!(input.components[0].value, 5);
    }

    #[test]
    fn parse_compound_timeout() {
        let input: TimeoutInput = parse_quote! {
            1 m + 30 s
        };
        assert_eq!(input.components.len(), 2);
    }
}
