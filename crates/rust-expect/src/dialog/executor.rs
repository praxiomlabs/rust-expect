//! Dialog execution engine.

use super::definition::{Dialog, DialogStep};
use crate::error::{ExpectError, Result};
use crate::expect::PatternSet;
use crate::session::Session;
use crate::Pattern;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    /// Execute a dialog on a session.
    ///
    /// This runs through the dialog steps, expecting patterns and sending responses.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::{Session, Dialog, DialogStep, DialogExecutor};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///
    ///     let dialog = Dialog::named("login")
    ///         .step(DialogStep::new("prompt")
    ///             .with_expect("$")
    ///             .with_send("echo hello\n"));
    ///
    ///     let executor = DialogExecutor::new();
    ///     let result = executor.execute(&mut session, &dialog).await?;
    ///     assert!(result.success);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A step times out without `continue_on_timeout` set
    /// - The session closes unexpectedly
    /// - An I/O error occurs
    pub async fn execute<T>(
        &self,
        session: &mut Session<T>,
        dialog: &Dialog,
    ) -> Result<DialogResult>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin + Send,
    {
        if dialog.is_empty() {
            return Ok(DialogResult {
                dialog_name: dialog.name.clone(),
                success: true,
                steps: Vec::new(),
                output: String::new(),
                error: None,
            });
        }

        let mut step_results = Vec::new();
        let mut total_output = String::new();
        let mut step_count = 0;

        // Determine starting step
        let mut current_step_idx = if let Some(ref entry) = dialog.entry {
            dialog.steps.iter().position(|s| &s.name == entry).unwrap_or(0)
        } else {
            0
        };

        loop {
            // Prevent infinite loops
            step_count += 1;
            if step_count > self.max_steps {
                return Ok(DialogResult {
                    dialog_name: dialog.name.clone(),
                    success: false,
                    steps: step_results,
                    output: total_output,
                    error: Some(format!("Exceeded maximum steps ({})", self.max_steps)),
                });
            }

            // Get current step
            let step = match dialog.steps.get(current_step_idx) {
                Some(s) => s,
                None => break, // No more steps
            };

            // Execute the step
            let step_result = self.execute_step(session, step, dialog).await?;
            let success = step_result.success;
            total_output.push_str(&step_result.output);

            // Determine next step
            let next_step = step_result.next_step.clone();
            step_results.push(step_result);

            if !success {
                return Ok(DialogResult {
                    dialog_name: dialog.name.clone(),
                    success: false,
                    steps: step_results,
                    output: total_output,
                    error: Some(format!("Step '{}' failed", step.name)),
                });
            }

            // Move to next step
            if let Some(next_name) = next_step {
                if let Some(idx) = dialog.steps.iter().position(|s| s.name == next_name) {
                    current_step_idx = idx;
                } else {
                    // Next step not found, end dialog
                    break;
                }
            } else {
                // No explicit next, try sequential
                current_step_idx += 1;
                if current_step_idx >= dialog.steps.len() {
                    break;
                }
            }
        }

        Ok(DialogResult {
            dialog_name: dialog.name.clone(),
            success: true,
            steps: step_results,
            output: total_output,
            error: None,
        })
    }

    /// Execute a single dialog step on a session.
    ///
    /// # Errors
    ///
    /// Returns an error if an I/O error occurs (timeouts are handled per-step).
    pub async fn execute_step<T>(
        &self,
        session: &mut Session<T>,
        step: &DialogStep,
        dialog: &Dialog,
    ) -> Result<StepResult>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin + Send,
    {
        let timeout = step.timeout.unwrap_or(self.default_timeout);
        let mut output = String::new();
        let mut matched_text = None;

        // Handle expect pattern if present
        if let Some(ref expect_pattern) = step.expect {
            let pattern = Pattern::literal(dialog.substitute(expect_pattern));
            let mut patterns = PatternSet::new();
            patterns.add(pattern).add(Pattern::timeout(timeout));

            match session.expect_any(&patterns).await {
                Ok(m) => {
                    output = m.before.clone();
                    matched_text = Some(m.matched.clone());
                }
                Err(ExpectError::Timeout { buffer, .. }) => {
                    if step.continue_on_timeout {
                        output = buffer;
                    } else {
                        return Ok(StepResult {
                            step_name: step.name.clone(),
                            success: false,
                            output: buffer,
                            matched: None,
                            send: None,
                            error: Some(format!(
                                "Timeout waiting for pattern '{}' after {:?}",
                                expect_pattern, timeout
                            )),
                            next_step: None,
                        });
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Check for branch conditions based on matched text
        let mut next_step = step.next.clone();
        if let Some(ref matched) = matched_text {
            for (branch_pattern, branch_target) in &step.branches {
                if matched.contains(branch_pattern) {
                    next_step = Some(branch_target.clone());
                    break;
                }
            }
        }

        // Handle send if present
        let substituted_send = if let Some(ref send_text) = step.send {
            let substituted = dialog.substitute(send_text);
            session.send_str(&substituted).await?;
            Some(substituted)
        } else {
            None
        };

        // Determine next step if not set
        if next_step.is_none() {
            next_step = dialog
                .steps
                .iter()
                .position(|s| s.name == step.name)
                .and_then(|i| dialog.steps.get(i + 1))
                .map(|s| s.name.clone());
        }

        Ok(StepResult {
            step_name: step.name.clone(),
            success: true,
            output,
            matched: matched_text,
            send: substituted_send,
            error: None,
            next_step,
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
