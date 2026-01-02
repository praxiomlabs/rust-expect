//! Dialog script macro implementation.
//!
//! This module implements the `dialog!` macro for defining interactive
//! dialog scripts with send/expect sequences.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{braced, Expr, Ident, LitStr, Result, Token};

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
pub fn expand(input: DialogInput) -> TokenStream {
    let steps: Vec<_> = input
        .steps
        .into_iter()
        .map(|step| match step {
            DialogStep::Send(send) => {
                let data = &send.data;
                if send.newline {
                    quote! {
                        rust_expect::dialog::DialogStep::SendLine(#data.to_string())
                    }
                } else {
                    quote! {
                        rust_expect::dialog::DialogStep::Send(#data.to_string())
                    }
                }
            }
            DialogStep::Expect(expect) => {
                let pattern = &expect.pattern;
                let timeout = expect
                    .timeout.map_or_else(|| quote! { None }, |t| quote! { Some(#t) });

                if expect.is_regex {
                    quote! {
                        rust_expect::dialog::DialogStep::ExpectRegex {
                            pattern: #pattern.to_string(),
                            timeout: #timeout,
                        }
                    }
                } else {
                    quote! {
                        rust_expect::dialog::DialogStep::Expect {
                            pattern: #pattern.to_string(),
                            timeout: #timeout,
                        }
                    }
                }
            }
            DialogStep::Wait(wait) => {
                let duration = &wait.duration;
                quote! {
                    rust_expect::dialog::DialogStep::Wait(#duration)
                }
            }
            DialogStep::Timeout(timeout) => {
                let duration = &timeout.duration;
                quote! {
                    rust_expect::dialog::DialogStep::SetTimeout(#duration)
                }
            }
        })
        .collect();

    quote! {
        rust_expect::dialog::Dialog::new(vec![#(#steps),*])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn parse_simple_dialog() {
        let input: DialogInput = parse_quote! {
            expect "login:";
            sendln "username"
        };
        assert_eq!(input.steps.len(), 2);
    }
}
