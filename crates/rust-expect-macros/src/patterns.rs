//! Pattern set macro implementation.
//!
//! This module implements the `patterns!` macro for defining sets of patterns
//! that can be matched against terminal output.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitStr, Result, Token, braced};

/// A single pattern definition.
#[allow(clippy::struct_field_names)]
pub struct Pattern {
    /// Pattern name (optional).
    pub name: Option<Ident>,
    /// The pattern expression (literal string or regex).
    pub kind: PatternKind,
    /// Optional action to execute when matched.
    pub action: Option<Expr>,
}

/// The kind of pattern.
pub enum PatternKind {
    /// Literal string match.
    Literal(LitStr),
    /// Regex pattern.
    Regex(LitStr),
    /// Glob pattern.
    Glob(LitStr),
}

impl Parse for Pattern {
    fn parse(input: ParseStream) -> Result<Self> {
        // Check for optional name: pattern syntax
        let name = if input.peek(Ident) && input.peek2(Token![:]) {
            let name: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            Some(name)
        } else {
            None
        };

        // Parse pattern kind
        let pattern = if input.peek(Ident) {
            let kind: Ident = input.parse()?;
            match kind.to_string().as_str() {
                "regex" | "re" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let lit: LitStr = content.parse()?;
                    PatternKind::Regex(lit)
                }
                "glob" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let lit: LitStr = content.parse()?;
                    PatternKind::Glob(lit)
                }
                _ => {
                    return Err(syn::Error::new(
                        kind.span(),
                        format!("unknown pattern type: {kind}"),
                    ));
                }
            }
        } else {
            // Literal string pattern
            let lit: LitStr = input.parse()?;
            PatternKind::Literal(lit)
        };

        // Check for optional action
        let action = if input.peek(Token![=>]) {
            let _: Token![=>] = input.parse()?;
            let expr: Expr = input.parse()?;
            Some(expr)
        } else {
            None
        };

        Ok(Self {
            name,
            kind: pattern,
            action,
        })
    }
}

/// The patterns! macro input.
pub struct PatternsInput {
    /// The list of patterns.
    pub patterns: Punctuated<Pattern, Token![,]>,
}

impl Parse for PatternsInput {
    fn parse(input: ParseStream) -> Result<Self> {
        // Handle braced or unbraced syntax
        let patterns = if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            Punctuated::parse_terminated(&content)?
        } else {
            Punctuated::parse_terminated(input)?
        };

        Ok(Self { patterns })
    }
}

/// Generate code for the patterns! macro.
pub fn expand(input: PatternsInput) -> TokenStream {
    let patterns: Vec<_> = input
        .patterns
        .into_iter()
        .enumerate()
        .map(|(idx, pattern)| {
            let name = pattern
                .name
                .map_or_else(|| quote! { None }, |n| quote! { Some(#n.to_string()) });

            let pattern_expr = match pattern.kind {
                PatternKind::Literal(lit) => {
                    quote! { rust_expect::pattern::PatternType::Literal(#lit.to_string()) }
                }
                PatternKind::Regex(lit) => {
                    let pattern_str = lit.value();
                    // Validate regex at compile time
                    if let Err(e) = regex::Regex::new(&pattern_str) {
                        return syn::Error::new(lit.span(), format!("invalid regex: {e}"))
                            .to_compile_error();
                    }
                    quote! { rust_expect::pattern::PatternType::Regex(#lit.to_string()) }
                }
                PatternKind::Glob(lit) => {
                    quote! { rust_expect::pattern::PatternType::Glob(#lit.to_string()) }
                }
            };

            let action_expr = pattern.action.map_or_else(
                || quote! { None::<Box<dyn Fn(&str)>> },
                |a| quote! { Some(Box::new(move |_| { #a })) },
            );

            quote! {
                rust_expect::pattern::Pattern {
                    name: #name,
                    index: #idx,
                    pattern: #pattern_expr,
                    action: #action_expr,
                }
            }
        })
        .collect();

    quote! {
        rust_expect::pattern::PatternSet::new(vec![#(#patterns),*])
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn parse_simple_pattern() {
        let input: PatternsInput = parse_quote! {
            "hello"
        };
        assert_eq!(input.patterns.len(), 1);
    }

    #[test]
    fn parse_multiple_patterns() {
        let input: PatternsInput = parse_quote! {
            "hello",
            "world"
        };
        assert_eq!(input.patterns.len(), 2);
    }
}
