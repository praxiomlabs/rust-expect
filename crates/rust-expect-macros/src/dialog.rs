//! Dialog script macro implementation.
//!
//! This module implements the `dialog!` macro for defining interactive
//! dialog scripts with send/expect sequences.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitStr, Result, Token, braced};

/// A single step in a dialog.
pub enum DialogStep {
    /// Send data to the terminal.
    Send(SendStep),
    /// Expect output from the terminal.
    Expect(ExpectStep),
    /// Wait for a duration.
    Wait(WaitStep),
    /// Set a timeout for subsequent operations.
    Timeout(TimeoutStep),
}

/// A send operation.
pub struct SendStep {
    /// The data to send.
    pub data: LitStr,
    /// Whether to send a newline after.
    pub newline: bool,
}

/// An expect operation.
pub struct ExpectStep {
    /// The pattern to match.
    pub pattern: LitStr,
    /// Whether this is a regex pattern.
    pub is_regex: bool,
    /// Optional timeout override.
    pub timeout: Option<Expr>,
}

/// A wait operation.
pub struct WaitStep {
    /// Duration expression.
    pub duration: Expr,
}

/// A timeout configuration.
pub struct TimeoutStep {
    /// Duration expression.
    pub duration: Expr,
}

impl Parse for DialogStep {
    fn parse(input: ParseStream) -> Result<Self> {
        let keyword: Ident = input.parse()?;

        match keyword.to_string().as_str() {
            "send" => {
                let data: LitStr = input.parse()?;
                Ok(Self::Send(SendStep {
                    data,
                    newline: false,
                }))
            }
            "sendln" | "send_line" => {
                let data: LitStr = input.parse()?;
                Ok(Self::Send(SendStep {
                    data,
                    newline: true,
                }))
            }
            "expect" => {
                let pattern: LitStr = input.parse()?;
                let timeout = if input.peek(Token![,]) {
                    let _: Token![,] = input.parse()?;
                    Some(input.parse()?)
                } else {
                    None
                };
                Ok(Self::Expect(ExpectStep {
                    pattern,
                    is_regex: false,
                    timeout,
                }))
            }
            "expect_re" | "expect_regex" => {
                let pattern: LitStr = input.parse()?;
                // Validate regex at compile time
                let pattern_str = pattern.value();
                if let Err(e) = regex::Regex::new(&pattern_str) {
                    return Err(syn::Error::new(
                        pattern.span(),
                        format!("invalid regex: {e}"),
                    ));
                }
                let timeout = if input.peek(Token![,]) {
                    let _: Token![,] = input.parse()?;
                    Some(input.parse()?)
                } else {
                    None
                };
                Ok(Self::Expect(ExpectStep {
                    pattern,
                    is_regex: true,
                    timeout,
                }))
            }
            "wait" | "sleep" => {
                let duration: Expr = input.parse()?;
                Ok(Self::Wait(WaitStep { duration }))
            }
            "timeout" => {
                let duration: Expr = input.parse()?;
                Ok(Self::Timeout(TimeoutStep { duration }))
            }
            other => Err(syn::Error::new(
                keyword.span(),
                format!("unknown dialog command: {other}"),
            )),
        }
    }
}

/// The dialog! macro input.
pub struct DialogInput {
    /// The steps in the dialog.
    pub steps: Punctuated<DialogStep, Token![;]>,
}

impl Parse for DialogInput {
    fn parse(input: ParseStream) -> Result<Self> {
        // Handle braced or unbraced syntax
        let steps = if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            Punctuated::parse_terminated(&content)?
        } else {
            Punctuated::parse_terminated(input)?
        };

        Ok(Self { steps })
    }
}

/// Generate code for the dialog! macro.
///
/// Expands to a `rust_expect::Dialog` built through its real builders. The
/// previous expansion treated `DialogStep` as an enum with `Send`, `Expect`,
/// `Wait` and `SetTimeout` variants and handed `Dialog::new` a vector; the
/// runtime type is a struct and `Dialog::new` takes no arguments, so nothing
/// this macro produced had ever compiled.
///
/// Two constructs the parser accepts have no runtime equivalent and are
/// rejected here rather than expanded into something that does not mean what
/// it says.
pub fn expand(input: DialogInput) -> TokenStream {
    let mut steps = Vec::with_capacity(input.steps.len());
    // `timeout <duration>;` applies to every expectation after it, until
    // another one replaces it. A step's own `, <duration>` still wins.
    let mut standing_timeout: Option<Expr> = None;

    for step in input.steps {
        match step {
            DialogStep::Send(send) => {
                let data = &send.data;
                // `sendln` appends a bare LF: a dialog step carries the text to
                // send and nothing else, so it cannot defer to the session's
                // configured line ending the way `Session::send_line` does.
                let text = if send.newline {
                    quote! { concat!(#data, "\n") }
                } else {
                    quote! { #data }
                };
                steps.push(quote! { .step(::rust_expect::DialogStep::send(#text)) });
            }
            DialogStep::Expect(expect) => {
                if expect.is_regex {
                    return syn::Error::new(
                        expect.pattern.span(),
                        "`dialog!` cannot express a regex expectation: dialog steps match their \
                         pattern literally. Match the regex directly with \
                         `session.expect(Pattern::regex(..))`.",
                    )
                    .to_compile_error();
                }

                let pattern = &expect.pattern;
                let timeout = expect.timeout.as_ref().or(standing_timeout.as_ref());
                steps.push(if let Some(timeout) = timeout {
                    quote! {
                        .step(::rust_expect::DialogStep::expect(#pattern).timeout(#timeout))
                    }
                } else {
                    quote! { .step(::rust_expect::DialogStep::expect(#pattern)) }
                });
            }
            DialogStep::Wait(wait) => {
                return syn::Error::new_spanned(
                    &wait.duration,
                    "`dialog!` has no wait step: a dialog is a sequence of expectations and \
                     sends, with no timing of its own. Sleep around `run_dialog`, or give the \
                     next expectation a timeout.",
                )
                .to_compile_error();
            }
            DialogStep::Timeout(timeout) => {
                standing_timeout = Some(timeout.duration);
            }
        }
    }

    quote! {
        ::rust_expect::Dialog::new() #(#steps)*
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn parse_simple_dialog() {
        let input: DialogInput = parse_quote! {
            expect "login:";
            sendln "username"
        };
        assert_eq!(input.steps.len(), 2);
    }
}
