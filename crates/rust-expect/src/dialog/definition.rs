//! Dialog definitions for scripted interactions.

use std::collections::HashMap;
use std::time::Duration;

use crate::types::ControlChar;

/// A dialog step definition.
#[derive(Debug, Clone, Default)]
pub struct DialogStep {
    /// Name of this step (optional for simple dialogs).
    pub name: String,
    /// Pattern to expect.
    pub expect: Option<String>,
    /// Response to send.
    pub send: Option<String>,
    /// Control character to send (alternative to text).
    pub send_control: Option<ControlChar>,
    /// Timeout for this step.
    pub timeout: Option<Duration>,
    /// Whether to continue on timeout.
    pub continue_on_timeout: bool,
    /// Next step name (for branching).
    pub next: Option<String>,
    /// Conditional branches.
    pub branches: HashMap<String, String>,
}

impl DialogStep {
    /// Create a new step with a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a step that expects a pattern (simple unnamed step).
    #[must_use]
    pub fn expect(pattern: impl Into<String>) -> Self {
        Self {
            expect: Some(pattern.into()),
            ..Default::default()
        }
    }

    /// Create a step that sends text (simple unnamed step).
    #[must_use]
    pub fn send(text: impl Into<String>) -> Self {
        Self {
            send: Some(text.into()),
            ..Default::default()
        }
    }

    /// Chain: set the pattern to expect (builder pattern).
    #[must_use]
    pub fn with_expect(mut self, pattern: impl Into<String>) -> Self {
        self.expect = Some(pattern.into());
        self
    }

    /// Chain: set the text to send (builder pattern).
    #[must_use]
    pub fn with_send(mut self, text: impl Into<String>) -> Self {
        self.send = Some(text.into());
        self
    }

    /// Chain: set a control character to send (e.g., Ctrl+C).
    #[must_use]
    pub fn with_send_control(mut self, ctrl: ControlChar) -> Self {
        self.send_control = Some(ctrl);
        self
    }

    /// Chain: set the text to send after expecting.
    /// Alias for `with_send`, for fluent API.
    #[must_use]
    pub fn then_send(mut self, text: impl Into<String>) -> Self {
        self.send = Some(text.into());
        self
    }

    /// Chain: set a control character to send after expecting.
    /// Alias for `with_send_control`, for fluent API.
    #[must_use]
    pub fn then_send_control(mut self, ctrl: ControlChar) -> Self {
        self.send_control = Some(ctrl);
        self
    }

    /// Set the timeout for this step.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the next step name.
    #[must_use]
    pub fn then(mut self, next: impl Into<String>) -> Self {
        self.next = Some(next.into());
        self
    }

    /// Add a conditional branch.
    #[must_use]
    pub fn branch(mut self, pattern: impl Into<String>, step: impl Into<String>) -> Self {
        self.branches.insert(pattern.into(), step.into());
        self
    }

    /// Set whether to continue on timeout.
    #[must_use]
    pub const fn continue_on_timeout(mut self, cont: bool) -> Self {
        self.continue_on_timeout = cont;
        self
    }

    /// Get the expect pattern.
    #[must_use]
    pub fn expect_pattern(&self) -> Option<&str> {
        self.expect.as_deref()
    }

    /// Get the send text.
    #[must_use]
    pub fn send_text(&self) -> Option<&str> {
        self.send.as_deref()
    }

    /// Get the control character to send.
    #[must_use]
    pub const fn send_control(&self) -> Option<ControlChar> {
        self.send_control
    }

    /// Get the timeout.
    #[must_use]
    pub const fn get_timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Check if should continue on timeout.
    #[must_use]
    pub const fn continues_on_timeout(&self) -> bool {
        self.continue_on_timeout
    }
}

/// A complete dialog definition.
#[derive(Debug, Clone, Default)]
pub struct Dialog {
    /// Name of the dialog.
    pub name: String,
    /// Description.
    pub description: String,
    /// Steps in the dialog.
    pub steps: Vec<DialogStep>,
    /// Entry point step name.
    pub entry: Option<String>,
    /// Variables for substitution.
    pub variables: HashMap<String, String>,
}

impl Dialog {
    /// Create a new empty dialog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new dialog with a name.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the description.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a step.
    #[must_use]
    pub fn step(mut self, step: DialogStep) -> Self {
        if self.entry.is_none() && !step.name.is_empty() {
            self.entry = Some(step.name.clone());
        }
        self.steps.push(step);
        self
    }

    /// Set a variable.
    #[must_use]
    pub fn variable(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// Set the entry point.
    #[must_use]
    pub fn entry_point(mut self, step: impl Into<String>) -> Self {
        self.entry = Some(step.into());
        self
    }

    /// Get the number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if the dialog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get the steps.
    #[must_use]
    pub fn steps(&self) -> &[DialogStep] {
        &self.steps
    }

    /// Get the variables.
    #[must_use]
    pub const fn variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    /// Get a step by name.
    #[must_use]
    pub fn get_step(&self, name: &str) -> Option<&DialogStep> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// Substitute variables in a string.
    #[must_use]
    pub fn substitute(&self, s: &str) -> String {
        let mut result = s.to_string();
        for (name, value) in &self.variables {
            result = result.replace(&format!("${{{name}}}"), value);
            result = result.replace(&format!("${name}"), value);
        }
        result
    }
}

/// A builder for creating dialogs.
#[derive(Debug, Clone, Default)]
pub struct DialogBuilder {
    dialog: Dialog,
}

impl DialogBuilder {
    /// Create a new builder (unnamed dialog).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new builder with a named dialog.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            dialog: Dialog::named(name),
        }
    }

    /// Add a step.
    #[must_use]
    pub fn step(mut self, step: DialogStep) -> Self {
        self.dialog = self.dialog.step(step);
        self
    }

    /// Add a variable (alias for var).
    #[must_use]
    pub fn variable(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.dialog = self.dialog.variable(name, value);
        self
    }

    /// Add a variable.
    #[must_use]
    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.dialog = self.dialog.variable(name, value);
        self
    }

    /// Add a simple expect-send step.
    #[must_use]
    pub fn expect_send(
        mut self,
        name: impl Into<String>,
        expect: impl Into<String>,
        send: impl Into<String>,
    ) -> Self {
        self.dialog = self
            .dialog
            .step(DialogStep::new(name).with_expect(expect).with_send(send));
        self
    }

    /// Build the dialog.
    #[must_use]
    pub fn build(self) -> Dialog {
        self.dialog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_basic() {
        let dialog = DialogBuilder::new()
            .step(DialogStep::expect("login:").then_send("admin"))
            .step(DialogStep::expect("password:").then_send("secret"))
            .var("USER", "admin")
            .build();

        assert_eq!(dialog.len(), 2);
        assert_eq!(dialog.substitute("${USER}"), "admin");
    }

    #[test]
    fn dialog_empty() {
        let dialog = Dialog::new();
        assert!(dialog.is_empty());
        assert_eq!(dialog.len(), 0);
    }

    #[test]
    fn dialog_named_steps() {
        let dialog = Dialog::named("login")
            .step(
                DialogStep::new("username")
                    .with_expect("login:")
                    .with_send("admin\n"),
            )
            .step(
                DialogStep::new("password")
                    .with_expect("password:")
                    .with_send("secret\n"),
            )
            .variable("USER", "admin");

        assert_eq!(dialog.name, "login");
        assert_eq!(dialog.steps.len(), 2);
        assert_eq!(dialog.substitute("${USER}"), "admin");
    }

    #[test]
    fn dialog_step_accessors() {
        let step = DialogStep::expect("prompt")
            .then_send("response")
            .timeout(Duration::from_secs(10));

        assert_eq!(step.expect_pattern(), Some("prompt"));
        assert_eq!(step.send_text(), Some("response"));
        assert_eq!(step.get_timeout(), Some(Duration::from_secs(10)));
    }

    #[test]
    fn dialog_variable_substitution() {
        let dialog = DialogBuilder::new()
            .var("name", "Alice")
            .var("greeting", "Hello")
            .build();

        assert_eq!(dialog.substitute("${greeting}, ${name}!"), "Hello, Alice!");
    }

    #[test]
    fn dialog_builder_named() {
        let dialog = DialogBuilder::named("test")
            .expect_send("step1", "prompt>", "command\n")
            .variable("VAR", "value")
            .build();

        assert_eq!(dialog.name, "test");
        assert_eq!(dialog.steps.len(), 1);
    }
}
