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
///
/// Expands to a `rust_expect::PatternSet` built through its real
/// constructors. The previous expansion named a `rust_expect::pattern` module,
/// a `PatternType` enum and a struct-shaped `Pattern`, none of which exist —
/// so nothing this macro produced had ever compiled.
pub fn expand(input: PatternsInput) -> TokenStream {
    let mut adds = Vec::with_capacity(input.patterns.len());

    for pattern in input.patterns {
        // A pattern set holds patterns and nothing else — there is no slot for
        // a handler — so an action has nowhere to go. Say that at the call
        // site instead of emitting code that cannot compile.
        if let Some(action) = pattern.action.as_ref() {
            return syn::Error::new_spanned(
                action,
                "`patterns!` does not support actions: a PatternSet carries patterns only. \
                 Register a handler with `Session::pattern_manager_mut()` and \
                 `PersistentPattern` instead.",
            )
            .to_compile_error();
        }

        let pattern_expr = match &pattern.kind {
            PatternKind::Literal(lit) => quote! { ::rust_expect::Pattern::literal(#lit) },
            PatternKind::Regex(lit) => {
                // Validate regex at compile time
                if let Err(e) = regex::Regex::new(&lit.value()) {
                    return syn::Error::new(lit.span(), format!("invalid regex: {e}"))
                        .to_compile_error();
                }
                quote! {
                    ::rust_expect::Pattern::regex(#lit)
                        .expect("regex was validated when `patterns!` expanded")
                }
            }
            PatternKind::Glob(lit) => quote! { ::rust_expect::Pattern::glob(#lit) },
        };

        adds.push(if let Some(name) = pattern.name.as_ref() {
            let name = name.to_string();
            quote! { set.add_named(#name, #pattern_expr); }
        } else {
            quote! { set.add(#pattern_expr); }
        });
    }

    quote! {
        {
            let mut set = ::rust_expect::PatternSet::new();
            #(#adds)*
            set
        }
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
