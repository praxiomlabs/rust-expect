//! Integration tests for dialog scripting.

use rust_expect::{Dialog, DialogBuilder, DialogStep};
use std::time::Duration;

#[test]
fn dialog_step_expect() {
    let step = DialogStep::expect("prompt");
    assert_eq!(step.expect_pattern(), Some("prompt"));
    assert!(step.send_text().is_none());
}

#[test]
fn dialog_step_send() {
    let step = DialogStep::send("data\n");
    assert!(step.expect_pattern().is_none());
    assert_eq!(step.send_text(), Some("data\n"));
}

#[test]
fn dialog_step_expect_then_send() {
    let step = DialogStep::expect("login:")
        .then_send("user\n");

    assert_eq!(step.expect_pattern(), Some("login:"));
    assert_eq!(step.send_text(), Some("user\n"));
}

#[test]
fn dialog_step_with_timeout() {
    let step = DialogStep::expect("prompt")
        .timeout(Duration::from_secs(10));

    assert_eq!(step.get_timeout(), Some(Duration::from_secs(10)));
}

#[test]
fn dialog_step_continue_on_timeout() {
    let step = DialogStep::expect("prompt")
        .continue_on_timeout(true);

    assert!(step.continues_on_timeout());
}

#[test]
fn dialog_step_new_named() {
    let step = DialogStep::new("login_step")
        .with_expect("login:")
        .with_send("admin\n");

    assert_eq!(step.name, "login_step");
    assert_eq!(step.expect_pattern(), Some("login:"));
    assert_eq!(step.send_text(), Some("admin\n"));
}

#[test]
fn dialog_step_branching() {
    let step = DialogStep::new("check_result")
        .with_expect("Result:")
        .branch("success", "handle_success")
        .branch("failure", "handle_failure")
        .then("cleanup");

    assert_eq!(step.branches.len(), 2);
    assert_eq!(step.next, Some("cleanup".to_string()));
}

#[test]
fn dialog_new() {
    let dialog = Dialog::new();
    assert!(dialog.is_empty());
    assert_eq!(dialog.len(), 0);
}

#[test]
fn dialog_default() {
    let dialog = Dialog::default();
    assert!(dialog.steps().is_empty());
}

#[test]
fn dialog_named() {
    let dialog = Dialog::named("login_dialog");
    assert_eq!(dialog.name, "login_dialog");
    assert!(dialog.is_empty());
}

#[test]
fn dialog_with_description() {
    let dialog = Dialog::named("login")
        .description("Handles SSH login prompts");

    assert_eq!(dialog.description, "Handles SSH login prompts");
}

#[test]
fn dialog_add_steps() {
    let dialog = Dialog::new()
        .step(DialogStep::expect("login:").then_send("admin\n"))
        .step(DialogStep::expect("password:").then_send("secret\n"));

    assert_eq!(dialog.len(), 2);
    assert_eq!(dialog.steps().len(), 2);
}

#[test]
fn dialog_variable_substitution() {
    let dialog = Dialog::new()
        .variable("USER", "admin")
        .variable("PASS", "secret123");

    assert_eq!(dialog.substitute("${USER}:${PASS}"), "admin:secret123");
}

#[test]
fn dialog_builder_basic() {
    let dialog = DialogBuilder::new()
        .step(DialogStep::expect("login:").then_send("user\n"))
        .step(DialogStep::expect("password:").then_send("pass\n"))
        .build();

    assert_eq!(dialog.steps().len(), 2);
}

#[test]
fn dialog_builder_expect_send() {
    let dialog = DialogBuilder::new()
        .expect_send("step1", "prompt>", "command1\n")
        .expect_send("step2", "prompt>", "command2\n")
        .build();

    assert_eq!(dialog.len(), 2);
}

#[test]
fn dialog_builder_named() {
    let dialog = DialogBuilder::named("ssh_login")
        .var("HOST", "example.com")
        .step(DialogStep::expect("$").then_send("ssh ${HOST}\n"))
        .build();

    assert_eq!(dialog.name, "ssh_login");
    assert_eq!(dialog.substitute("${HOST}"), "example.com");
}

#[test]
fn dialog_get_step() {
    let dialog = Dialog::new()
        .step(DialogStep::new("step1").with_expect("prompt1"))
        .step(DialogStep::new("step2").with_expect("prompt2"));

    assert!(dialog.get_step("step1").is_some());
    assert!(dialog.get_step("step2").is_some());
    assert!(dialog.get_step("nonexistent").is_none());
}

#[test]
fn dialog_entry_point() {
    let dialog = Dialog::new()
        .step(DialogStep::new("start").with_expect("begin"))
        .entry_point("start");

    assert_eq!(dialog.entry, Some("start".to_string()));
}
