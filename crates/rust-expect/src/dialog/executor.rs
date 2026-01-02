//! Dialog execution engine.

use super::definition::{Dialog, DialogStep};
use crate::Pattern;
use std::time::Duration;

/// Result of executing a dialog step.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Name of the step.
    pub step_name: String,
    /// Whether the step succeeded.
    pub success: bool,
    /// Output captured before the match.
    pub output: String,
    /// The matched text.
    pub matched: Option<String>,
    /// The text that was/will be sent (after variable substitution).
    pub send: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Name of the next step to execute.
    pub next_step: Option<String>,
}

/// Result of executing a complete dialog.
#[derive(Debug, Clone)]
pub struct DialogResult {
    /// Name of the dialog.
    pub dialog_name: String,
    /// Whether the dialog succeeded.
    pub success: bool,
    /// Results of each step.
    pub steps: Vec<StepResult>,
    /// Total output captured.
    pub output: String,
    /// Error message if failed.
    pub error: Option<String>,
}

impl DialogResult {
    /// Check if all steps succeeded.
    #[must_use]
    pub fn all_success(&self) -> bool {
        self.steps.iter().all(|s| s.success)
    }

    /// Get the last step result.
    #[must_use]
    pub fn last_step(&self) -> Option<&StepResult> {
        self.steps.last()
    }

    /// Get a step by name.
    #[must_use]
    pub fn get_step(&self, name: &str) -> Option<&StepResult> {
        self.steps.iter().find(|s| s.step_name == name)
    }
}

/// Dialog execution state.
#[derive(Debug)]
pub struct DialogExecutor {
    /// Maximum number of steps to execute.
    pub max_steps: usize,
    /// Default timeout for steps without explicit timeout.
    pub default_timeout: Duration,
}

impl Default for DialogExecutor {
    fn default() -> Self {
        Self {
            max_steps: 100,
            default_timeout: Duration::from_secs(30),
        }
    }
}

impl DialogExecutor {
    /// Create a new executor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of steps.
    #[must_use]
    pub const fn max_steps(mut self, max: usize) -> Self {
        self.max_steps = max;
        self
    }

    /// Set the default timeout.
    #[must_use]
    pub const fn default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Execute a single step (synchronously - for testing).
    ///
    /// This method prepares a step for execution by:
    /// - Substituting variables in the send text
    /// - Determining the next step to execute
    ///
    /// Note: This is a synchronous helper primarily for testing. For actual
    /// dialog execution, use the async session-based execution methods.
    #[must_use]
    pub fn execute_step_sync(
        &self,
        step: &DialogStep,
        dialog: &Dialog,
        _buffer: &str,
    ) -> StepResult {
        let substituted_send = step.send.as_ref().map(|s| dialog.substitute(s));

        StepResult {
            step_name: step.name.clone(),
            success: true,
            output: String::new(),
            matched: step.expect.clone(),
            send: substituted_send,
            error: None,
            next_step: step.next.clone().or_else(|| {
                // Find next step in sequence
                dialog
                    .steps
                    .iter()
                    .position(|s| s.name == step.name)
                    .and_then(|i| dialog.steps.get(i + 1))
                    .map(|s| s.name.clone())
            }),
        }
    }

    /// Get the pattern for a step.
    #[must_use]
    pub fn step_pattern(&self, step: &DialogStep, dialog: &Dialog) -> Option<Pattern> {
        step.expect.as_ref().map(|e| {
            Pattern::literal(dialog.substitute(e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_default() {
        let executor = DialogExecutor::new();
        assert_eq!(executor.max_steps, 100);
    }

    #[test]
    fn step_result_success() {
        let result = StepResult {
            step_name: "test".to_string(),
            success: true,
            output: "output".to_string(),
            matched: Some("match".to_string()),
            send: Some("hello".to_string()),
            error: None,
            next_step: None,
        };
        assert!(result.success);
        assert_eq!(result.send, Some("hello".to_string()));
    }

    #[test]
    fn step_result_with_substitution() {
        use super::super::definition::{Dialog, DialogStep};

        let dialog = Dialog::named("test_dialog")
            .variable("username", "admin")
            .step(
                DialogStep::new("login")
                    .with_expect("Username:")
                    .with_send("${username}"),
            );

        let executor = DialogExecutor::new();
        let step = &dialog.steps[0];
        let result = executor.execute_step_sync(step, &dialog, "");

        assert_eq!(result.step_name, "login");
        assert_eq!(result.send, Some("admin".to_string()));
    }
}
