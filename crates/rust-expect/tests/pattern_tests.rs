//! Pattern matching tests.

use rust_expect::expect::{Pattern, PatternSet};

#[test]
fn literal_pattern_matches() {
    let pattern = Pattern::literal("hello");
    assert!(pattern.matches("hello world").is_some());
    assert!(pattern.matches("goodbye").is_none());
}

#[test]
fn regex_pattern_matches() {
    let pattern = Pattern::regex(r"\d+").unwrap();
    assert!(pattern.matches("test123").is_some());
    assert!(pattern.matches("nodigits").is_none());
}

#[test]
fn pattern_set_matches_any() {
    let mut set = PatternSet::new();
    set.add(Pattern::literal("login:"))
        .add(Pattern::literal("password:"));

    assert!(set.find_match("Please enter login:").is_some());
    assert!(set.find_match("Enter password:").is_some());
    assert!(set.find_match("unknown prompt").is_none());
}

#[test]
fn pattern_special_types() {
    let eof = Pattern::eof();
    assert!(eof.is_eof());

    let timeout = Pattern::timeout(std::time::Duration::from_secs(5));
    assert!(timeout.is_timeout());
    assert_eq!(
        timeout.timeout_duration(),
        Some(std::time::Duration::from_secs(5))
    );
}

#[test]
fn pattern_extract_match() {
    let pattern = Pattern::regex(r"user: (\w+)").unwrap();
    let result = pattern.matches("user: alice");

    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.start, 0);
}

#[test]
fn pattern_set_returns_index() {
    let mut set = PatternSet::new();
    set.add(Pattern::literal("first"))
        .add(Pattern::literal("second"))
        .add(Pattern::literal("third"));

    let result = set.find_match("contains second pattern");
    assert!(result.is_some());
    let (idx, _) = result.unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn empty_pattern_set() {
    let set = PatternSet::new();
    assert!(set.find_match("anything").is_none());
}

#[test]
fn glob_pattern() {
    let pattern = Pattern::glob("*.txt");
    assert!(pattern.matches("file.txt").is_some());
    assert!(pattern.matches("file.rs").is_none());
}

// =============================================================================
// Invalid regex and error path tests
// =============================================================================

#[test]
fn invalid_regex_returns_error() {
    // Unclosed bracket
    let result = Pattern::regex(r"[invalid");
    assert!(result.is_err());

    // Unclosed parenthesis
    let result = Pattern::regex(r"(unclosed");
    assert!(result.is_err());

    // Invalid repetition
    let result = Pattern::regex(r"*invalid");
    assert!(result.is_err());

    // Unclosed repetition
    let result = Pattern::regex(r"x{1,");
    assert!(result.is_err());
}

#[test]
fn invalid_regex_error_message_is_useful() {
    let result = Pattern::regex(r"[invalid");
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The error message should mention something about the invalid pattern
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

#[test]
fn empty_pattern_literal() {
    // Empty literal should still work (matches at start)
    let pattern = Pattern::literal("");
    assert!(pattern.matches("anything").is_some());
}

#[test]
fn empty_regex_pattern() {
    // Empty regex should be valid (matches at start)
    let pattern = Pattern::regex(r"").unwrap();
    assert!(pattern.matches("anything").is_some());
}

#[test]
fn regex_with_special_characters() {
    // Regex with special chars that need escaping
    let pattern = Pattern::regex(r"\$\[\]").unwrap();
    assert!(pattern.matches("test$[]more").is_some());
    assert!(pattern.matches("test$more").is_none());
}

#[test]
fn pattern_clone_works() {
    let original = Pattern::regex(r"\d+").unwrap();
    let cloned = original.clone();
    // Verify both original and clone work independently
    assert!(original.matches("456").is_some());
    assert!(cloned.matches("123").is_some());
}

#[test]
fn pattern_set_add_chain() {
    let mut set = PatternSet::new();
    set.add(Pattern::literal("a"))
        .add(Pattern::literal("b"))
        .add(Pattern::literal("c"));

    assert!(set.find_match("a").is_some());
    assert!(set.find_match("b").is_some());
    assert!(set.find_match("c").is_some());
    assert_eq!(set.len(), 3);
}

#[test]
fn convenience_patterns() {
    // Test shell_prompt convenience method
    let prompt = Pattern::shell_prompt();
    assert!(prompt.matches("user@host:~$ ").is_some());
    assert!(prompt.matches("root@server# ").is_some());

    // Test password_prompt convenience method
    let pwd = Pattern::password_prompt();
    assert!(pwd.matches("Password: ").is_some());
    assert!(pwd.matches("password: ").is_some());

    // Test login_prompt convenience method
    let login = Pattern::login_prompt();
    assert!(login.matches("login: ").is_some());
    assert!(login.matches("Username: ").is_some());
}

#[test]
fn ipv4_pattern_validates() {
    let ipv4 = Pattern::ipv4().unwrap();

    // Valid IPs
    assert!(ipv4.matches("192.168.1.1").is_some());
    assert!(ipv4.matches("10.0.0.1").is_some());
    assert!(ipv4.matches("255.255.255.255").is_some());

    // Invalid IPs (no match)
    assert!(ipv4.matches("256.1.1.1").is_none());
    assert!(ipv4.matches("not an ip").is_none());
}

#[test]
fn email_pattern_validates() {
    let email = Pattern::email().unwrap();

    // Valid emails
    assert!(email.matches("user@example.com").is_some());
    assert!(email.matches("test.user+tag@domain.org").is_some());

    // Invalid emails (no match)
    assert!(email.matches("not-an-email").is_none());
    assert!(email.matches("@missing-local.com").is_none());
}

#[test]
fn error_indicator_pattern() {
    let error_pat = Pattern::error_indicator();
    assert!(error_pat.matches("Error: something failed").is_some());
    assert!(error_pat.matches("FAILED: test").is_some());
    assert!(error_pat.matches("fatal: cannot continue").is_some());
}

#[test]
fn success_indicator_pattern() {
    let success_pat = Pattern::success_indicator();
    assert!(success_pat.matches("Success!").is_some());
    assert!(success_pat.matches("OK").is_some());
    assert!(success_pat.matches("PASSED").is_some());
    assert!(success_pat.matches("complete").is_some());
    // "done" is not included in the pattern
    assert!(success_pat.matches("done").is_none());
}
