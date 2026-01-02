//! Dialog execution tests.

use rust_expect::dialog::{Dialog, DialogBuilder, DialogStep};
use std::time::Duration;

#[test]
fn dialog_builder_creates_steps() {
    let dialog = DialogBuilder::new()
        .step(DialogStep::expect("login:").then_send("admin"))
        .step(DialogStep::expect("password:").then_send("secret"))
        .step(DialogStep::expect("$"))
        .build();

    assert_eq!(dialog.len(), 3);
}

#[test]
fn dialog_with_variables() {
    let dialog = DialogBuilder::new()
        .var("username", "admin")
        .var("password", "secret123")
        .step(DialogStep::expect("login:").then_send("${username}"))
        .step(DialogStep::expect("password:").then_send("${password}"))
        .build();

    let vars = dialog.variables();
    assert_eq!(vars.get("username"), Some(&"admin".to_string()));
    assert_eq!(vars.get("password"), Some(&"secret123".to_string()));
}

#[test]
fn dialog_empty() {
    let dialog = Dialog::new();
    assert!(dialog.is_empty());
    assert_eq!(dialog.len(), 0);
}

#[test]
fn dialog_step_creation() {
    let expect_step = DialogStep::expect("pattern");
    assert!(expect_step.expect_pattern().is_some());

    let send_step = DialogStep::send("text");
    assert!(send_step.send_text().is_some());
}

#[test]
fn dialog_variable_substitution() {
    let dialog = DialogBuilder::new()
        .var("name", "Alice")
        .build();

    let result = dialog.substitute("Hello, ${name}!");
    assert_eq!(result, "Hello, Alice!");
}

#[test]
fn dialog_nested_variable() {
    let dialog = DialogBuilder::new()
        .var("greeting", "Hello")
        .var("name", "World")
        .build();

    let result = dialog.substitute("${greeting}, ${name}!");
    assert_eq!(result, "Hello, World!");
}

#[test]
fn dialog_step_with_timeout() {
    let step = DialogStep::expect("prompt")
        .timeout(Duration::from_secs(10));

    assert_eq!(step.get_timeout(), Some(Duration::from_secs(10)));
}
