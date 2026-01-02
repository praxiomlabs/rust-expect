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
    let set = PatternSet::new()
        .add(Pattern::literal("login:"))
        .add(Pattern::literal("password:"));

    assert!(set.find("Please enter login:").is_some());
    assert!(set.find("Enter password:").is_some());
    assert!(set.find("unknown prompt").is_none());
}

#[test]
fn pattern_special_types() {
    let eof = Pattern::eof();
    assert!(eof.is_eof());

    let timeout = Pattern::timeout(std::time::Duration::from_secs(5));
    assert!(timeout.is_timeout());
    assert_eq!(timeout.timeout_duration(), Some(std::time::Duration::from_secs(5)));
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
    let set = PatternSet::new()
        .add(Pattern::literal("first"))
        .add(Pattern::literal("second"))
        .add(Pattern::literal("third"));

    let result = set.find("contains second pattern");
    assert!(result.is_some());
    let (idx, _) = result.unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn empty_pattern_set() {
    let set = PatternSet::new();
    assert!(set.find("anything").is_none());
}

#[test]
fn glob_pattern() {
    let pattern = Pattern::glob("*.txt");
    assert!(pattern.matches("file.txt").is_some());
    assert!(pattern.matches("file.rs").is_none());
}
