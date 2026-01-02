//! rust-expect-macros: Procedural macros for rust-expect
//!
//! This crate provides compile-time macros for the rust-expect terminal automation library:
//!
//! - [`patterns!`] - Define pattern sets for expect operations
//! - [`regex!`] - Compile-time validated regex patterns
//! - [`dialog!`] - Define interactive dialog scripts
//! - [`timeout!`] - Parse timeout duration specifications
//!
//! # Example: Pattern Matching
//!
//! ```ignore
//! use rust_expect_macros::patterns;
//!
//! let patterns = patterns! {
//!     "login:",
//!     "password:",
//!     regex(r"\$\s*$"),
//! };
//! ```
//!
//! # Example: Dialog Script
//!
//! ```ignore
//! use rust_expect_macros::dialog;
//!
//! let script = dialog! {
//!     expect "login:";
//!     sendln "admin";
//!     expect "password:";
//!     sendln "secret";
//!     expect_re r"\$\s*$"
//! };
//! ```
//!
//! # Example: Validated Regex
//!
//! ```ignore
//! use rust_expect_macros::regex;
//!
//! // Compile-time validated regex
//! let prompt = regex!(r"^\w+@\w+:\S+\$\s*$");
//! ```
//!
//! # Example: Human-Readable Timeout
//!
//! ```ignore
//! use rust_expect_macros::timeout;
//!
//! let duration = timeout!(5 s);
//! let long_timeout = timeout!(2 m + 30 s);
//! ```

// In proc-macro crates, passing parsed input by value is idiomatic
#![allow(clippy::needless_pass_by_value)]

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod dialog;
mod patterns;
mod regex;
mod timeout;

/// Define a set of patterns for use with expect operations.
///
/// This macro creates a `PatternSet` with compile-time validated patterns.
///
/// # Syntax
///
/// ```ignore
/// patterns! {
///     "literal pattern",
///     name: "named pattern",
///     regex(r"regex\s+pattern"),
///     glob("glob*pattern"),
///     "pattern" => action_expression,
/// }
/// ```
///
/// # Examples
///
/// ```ignore
/// let login_patterns = patterns! {
///     login: "login:",
///     password: "password:",
///     prompt: regex(r"\$\s*$"),
/// };
///
/// // Use with session.expect()
/// let matched = session.expect(&login_patterns).await?;
/// ```
#[proc_macro]
pub fn patterns(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as patterns::PatternsInput);
    patterns::expand(input).into()
}

/// Compile-time validated regex pattern.
///
/// Creates a lazily-initialized `regex::Regex` that is validated at compile time.
/// Invalid regex patterns will cause a compilation error.
///
/// # Examples
///
/// ```ignore
/// use rust_expect_macros::regex;
///
/// // Valid regex - compiles successfully
/// let prompt = regex!(r"^\w+@\w+:\S+\$\s*$");
///
/// // Invalid regex - compilation error
/// // let bad = regex!(r"[invalid");
/// ```
#[proc_macro]
pub fn regex(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as regex::RegexInput);
    regex::expand(input).into()
}

/// Define an interactive dialog script.
///
/// Creates a `Dialog` that can be executed against a session, automating
/// send/expect sequences.
///
/// # Commands
///
/// - `send "text"` - Send text without newline
/// - `sendln "text"` - Send text with newline
/// - `expect "pattern"` - Wait for literal pattern
/// - `expect_re "regex"` - Wait for regex pattern (validated at compile time)
/// - `wait duration` - Wait for a duration
/// - `timeout duration` - Set timeout for subsequent operations
///
/// # Examples
///
/// ```ignore
/// use rust_expect_macros::dialog;
/// use std::time::Duration;
///
/// let login_script = dialog! {
///     timeout Duration::from_secs(30);
///     expect "login:";
///     sendln "admin";
///     expect "password:";
///     sendln "secret123";
///     expect_re r"\$\s*$"
/// };
///
/// // Execute the dialog
/// session.run_dialog(&login_script).await?;
/// ```
#[proc_macro]
pub fn dialog(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as dialog::DialogInput);
    dialog::expand(input).into()
}

/// Parse a human-readable timeout specification.
///
/// Creates a `std::time::Duration` from a human-readable format.
///
/// # Supported Units
///
/// - `ns`, `nanos`, `nanoseconds` - Nanoseconds
/// - `us`, `micros`, `microseconds` - Microseconds
/// - `ms`, `millis`, `milliseconds` - Milliseconds
/// - `s`, `sec`, `secs`, `seconds` - Seconds
/// - `m`, `min`, `mins`, `minutes` - Minutes
/// - `h`, `hr`, `hrs`, `hours` - Hours
///
/// # Examples
///
/// ```ignore
/// use rust_expect_macros::timeout;
///
/// let short = timeout!(100 ms);
/// let medium = timeout!(5 s);
/// let long = timeout!(2 m);
/// let compound = timeout!(1 m + 30 s + 500 ms);
/// ```
#[proc_macro]
pub fn timeout(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as timeout::TimeoutInput);
    timeout::expand(input).into()
}
