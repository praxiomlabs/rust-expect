//! Compile-time validated regex macro implementation.
//!
//! This module implements the `regex!` macro for creating regex patterns
//! that are validated at compile time.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Result};

/// The regex! macro input.
pub struct RegexInput {
    /// The regex pattern string.
    pub pattern: LitStr,
}

impl Parse for RegexInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            pattern: input.parse()?,
        })
    }
}

/// Generate code for the regex! macro.
pub fn expand(input: RegexInput) -> TokenStream {
    let pattern_str = input.pattern.value();

    // Validate the regex at compile time
    if let Err(e) = regex::Regex::new(&pattern_str) {
        return syn::Error::new(input.pattern.span(), format!("invalid regex: {e}"))
            .to_compile_error();
    }

    let lit = &input.pattern;

    quote! {
        {
            // Use once_cell for lazy static initialization without unsafe
            static REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            REGEX.get_or_init(|| {
                // SAFETY: We validated the regex at compile time
                regex::Regex::new(#lit).expect("regex was validated at compile time")
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn parse_simple_regex() {
        let input: RegexInput = parse_quote! {
            r"hello\s+world"
        };
        assert_eq!(input.pattern.value(), r"hello\s+world");
    }
}
