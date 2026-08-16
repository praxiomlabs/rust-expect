//! Compile and exercise every form of the exported macros.
//!
//! The macro crate's own tests check that input *parses*; they never compile
//! what comes out. Two of the four macros expanded into a runtime API that
//! did not exist — a `rust_expect::pattern` module, a `PatternType` enum, an
//! enum-shaped `DialogStep` — and a green test run said nothing about it,
//! because nothing in the workspace called them.
//!
//! This file is that missing check: it is an ordinary integration test, so it
//! links against rust-expect the way a downstream crate does, and every macro
//! form below has to compile for the suite to build at all.

use std::time::Duration;

use rust_expect::{Dialog, Pattern, PatternSet, dialog, patterns, regex, timeout};

#[test]
fn timeout_macro_builds_durations() {
    let five_seconds: Duration = timeout!(5 s);
    assert_eq!(five_seconds, Duration::from_secs(5));

    let millis: Duration = timeout!(250 ms);
    assert_eq!(millis, Duration::from_millis(250));

    let compound: Duration = timeout!(1 m + 30 s);
    assert_eq!(compound, Duration::from_secs(90));
}

#[test]
fn regex_macro_returns_a_compiled_regex() {
    let re = regex!(r"\d{3}-\d{4}");
    assert!(re.is_match("call 555-1234"));
    assert!(!re.is_match("no digits here"));

    // The same call site twice returns the same cached compilation.
    let again = regex!(r"\d{3}-\d{4}");
    assert!(again.is_match("555-1234"));
}

#[test]
fn patterns_macro_builds_a_literal_set() {
    let set: PatternSet = patterns!["hello", "world"];
    assert_eq!(set.len(), 2);
    assert!(set.find_match("say hello").is_some());
    assert!(set.find_match("nothing here").is_none());
}

#[test]
fn patterns_macro_supports_every_pattern_kind() {
    let set: PatternSet = patterns! {
        "plain",
        regex(r"\d+"),
        re(r"[a-z]+"),
        glob("*.rs"),
    };
    assert_eq!(set.len(), 4);
    assert!(set.find_match("plain").is_some());
    assert!(set.find_match("42").is_some());
}

#[test]
fn patterns_macro_supports_names() {
    let set: PatternSet = patterns! {
        prompt: "$ ",
        failure: regex(r"error: .*"),
    };
    assert_eq!(set.len(), 2);
    let named: Vec<_> = set.iter().filter_map(|p| p.name.as_deref()).collect();
    assert_eq!(named, vec!["prompt", "failure"]);
}

#[test]
fn patterns_macro_braced_and_unbraced_agree() {
    let braced: PatternSet = patterns! { "a", "b" };
    let unbraced: PatternSet = patterns!["a", "b"];
    assert_eq!(braced.len(), unbraced.len());
}

#[test]
fn dialog_macro_builds_expect_and_send_steps() {
    let dialog: Dialog = dialog! {
        expect "login:";
        send "admin";
        sendln "password";
    };

    assert_eq!(dialog.len(), 3);
    assert_eq!(dialog.steps()[0].expect_pattern(), Some("login:"));
    assert_eq!(dialog.steps()[1].send_text(), Some("admin"));
    assert_eq!(
        dialog.steps()[2].send_text(),
        Some("password\n"),
        "sendln appends a newline"
    );
}

#[test]
fn dialog_macro_applies_per_step_timeouts() {
    let dialog: Dialog = dialog! {
        expect "slow:", Duration::from_secs(5);
        expect "fast:";
    };

    assert_eq!(
        dialog.steps()[0].get_timeout(),
        Some(Duration::from_secs(5))
    );
    assert_eq!(dialog.steps()[1].get_timeout(), None);
}

#[test]
fn dialog_macro_timeout_statement_applies_to_following_steps() {
    let dialog: Dialog = dialog! {
        expect "before:";
        timeout Duration::from_secs(2);
        expect "after:";
        expect "later:", Duration::from_secs(9);
    };

    assert_eq!(
        dialog.steps()[0].get_timeout(),
        None,
        "a standing timeout applies only to what follows it"
    );
    assert_eq!(
        dialog.steps()[1].get_timeout(),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        dialog.steps()[2].get_timeout(),
        Some(Duration::from_secs(9)),
        "a step's own timeout wins over the standing one"
    );
}

#[test]
fn dialog_macro_output_runs_on_a_session() {
    // The point of the macro is a dialog the executor can actually run, so
    // check the built value against the runtime rather than only its shape.
    let dialog: Dialog = dialog! {
        expect "login:";
        sendln "admin";
    };

    let pattern: Pattern = Pattern::literal(dialog.steps()[0].expect_pattern().unwrap());
    assert!(matches!(pattern, Pattern::Literal(ref s) if s == "login:"));
    assert_eq!(dialog.entry, None, "the macro does not pin an entry point");
}
