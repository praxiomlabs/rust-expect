//! Dialog execution engine.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::definition::{Dialog, DialogStep};
use crate::Pattern;
use crate::error::{ExpectError, Result};
use crate::expect::PatternSet;
use crate::session::Session;

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
    /// Name of the step this one explicitly branched to, from `next` or a
    /// matched branch condition. `None` when execution simply falls through to
    /// the following step, which is resolved by position rather than by name.
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

    /// Get the pattern for a step.
    #[must_use]
    pub fn step_pattern(&self, step: &DialogStep, dialog: &Dialog) -> Option<Pattern> {
        step.expect
            .as_ref()
            .map(|e| Pattern::literal(dialog.substitute(e)))
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

        // Determine starting step. An entry point naming a step that does not
        // exist is the same defect as an unknown branch target: silently
        // starting at step 0 runs a different dialog than the caller asked for.
        let mut current_step_idx = 0;
        if let Some(ref entry) = dialog.entry {
            match dialog.steps.iter().position(|s| &s.name == entry) {
                Some(idx) => current_step_idx = idx,
                None => {
                    return Ok(DialogResult {
                        dialog_name: dialog.name.clone(),
                        success: false,
                        steps: Vec::new(),
                        output: String::new(),
                        error: Some(format!(
                            "Entry point '{entry}' is not a step in this dialog"
                        )),
                    });
                }
            }
        }

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
            let Some(step) = dialog.steps.get(current_step_idx) else {
                break; // No more steps
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
                    // A branch or `next` naming a step that does not exist is a
                    // broken dialog, not a finished one. Reporting success here
                    // made a typo'd target indistinguishable from completion.
                    return Ok(DialogResult {
                        dialog_name: dialog.name.clone(),
                        success: false,
                        steps: step_results,
                        output: total_output,
                        error: Some(format!(
                            "Step '{}' branches to unknown step '{next_name}'",
                            step.name
                        )),
                    });
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
                    output.clone_from(&m.before);
                    matched_text = Some(m.matched);
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
                                "Timeout waiting for pattern '{expect_pattern}' after {timeout:?}"
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

        // Handle send if present (text or control character)
        let substituted_send = if let Some(ref send_text) = step.send {
            let substituted = dialog.substitute(send_text);
            session.send_str(&substituted).await?;
            Some(substituted)
        } else if let Some(ctrl) = step.send_control {
            session.send_control(ctrl).await?;
            Some(format!("<{ctrl:?}>"))
        } else {
            None
        };

        // Sequential advancement is deliberately *not* resolved here. It used
        // to be filled in by looking up this step's own name to find its
        // successor, which sends every unnamed step (name `""`) back to the
        // first unnamed step in the dialog. `execute` advances by index when
        // this is `None`, which is correct whether or not steps are named.

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
}

/// Dialog execution driven through the real expect loop over a mock
/// transport. The builder-only tests above cannot see how steps advance.
#[cfg(all(test, feature = "mock"))]
mod execution_tests {
    use std::time::Duration;

    use super::super::definition::{Dialog, DialogStep};
    use super::DialogExecutor;
    use crate::config::SessionConfig;
    use crate::mock::MockTransport;
    use crate::session::Session;

    /// Build a session over a mock transport pre-loaded with `output`, keeping
    /// a cloned handle for reading back what the dialog sent.
    fn session_with(output: &str) -> (Session<MockTransport>, MockTransport) {
        let transport = MockTransport::new();
        let handle = transport.clone();
        handle.queue_output_str(output);
        let session = Session::new(transport, SessionConfig::default());
        (session, handle)
    }

    fn short_executor() -> DialogExecutor {
        DialogExecutor::new().default_timeout(Duration::from_millis(200))
    }

    /// Unnamed steps all carry the name `""`, and sequential advancement used
    /// to be resolved by looking that name up — which always found step 0. A
    /// dialog of unnamed steps therefore re-ran its first step instead of
    /// walking forward.
    #[tokio::test]
    async fn unnamed_steps_advance_sequentially() {
        let (mut session, handle) = session_with("one two three ");

        let dialog = Dialog::named("d")
            .step(DialogStep::expect("one").then_send("a\n"))
            .step(DialogStep::expect("two").then_send("b\n"))
            .step(DialogStep::expect("three").then_send("c\n"));

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(result.success, "dialog failed: {:?}", result.error);
        assert_eq!(result.steps.len(), 3, "every unnamed step should run once");
        assert_eq!(handle.take_input_str(), "a\nb\nc\n");
    }

    /// Named steps that fall through (no `next`, no branches) must advance the
    /// same way, by position rather than by name.
    #[tokio::test]
    async fn named_steps_advance_sequentially() {
        let (mut session, handle) = session_with("one two ");

        let dialog = Dialog::named("d")
            .step(DialogStep::new("first").with_expect("one").with_send("a\n"))
            .step(
                DialogStep::new("second")
                    .with_expect("two")
                    .with_send("b\n"),
            );

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(result.success, "dialog failed: {:?}", result.error);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(handle.take_input_str(), "a\nb\n");
    }

    /// An explicit `next` still jumps by name, skipping the step in between.
    #[tokio::test]
    async fn explicit_next_jumps() {
        let (mut session, handle) = session_with("start end ");

        let mut first = DialogStep::new("first")
            .with_expect("start")
            .with_send("a\n");
        first.next = Some("third".to_string());
        let dialog = Dialog::named("d")
            .step(first)
            .step(DialogStep::new("second").with_send("SKIPPED\n"))
            .step(DialogStep::new("third").with_expect("end").with_send("c\n"));

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(result.success, "dialog failed: {:?}", result.error);
        let sent = handle.take_input_str();
        assert_eq!(sent, "a\nc\n", "the skipped step must not have sent");
    }

    /// A `next` naming a step that does not exist used to end the dialog and
    /// report success, so a typo'd target looked exactly like completion.
    #[tokio::test]
    async fn unknown_branch_target_fails() {
        let (mut session, _handle) = session_with("hello ");

        let mut first = DialogStep::new("first").with_expect("hello");
        first.next = Some("does-not-exist".to_string());
        let dialog = Dialog::named("d").step(first);

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(!result.success, "unknown branch target must not succeed");
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|e| e.contains("does-not-exist")),
            "error should name the missing step, got {:?}",
            result.error
        );
    }

    /// Variables in a step's send text are substituted on the way out, which
    /// used to be checked through a sync helper that only *prepared* a step.
    /// Assert it against what actually reaches the transport.
    #[tokio::test]
    async fn send_text_is_variable_substituted() {
        let (mut session, handle) = session_with("Username: ");

        let dialog = Dialog::named("login").variable("username", "admin").step(
            DialogStep::new("login")
                .with_expect("Username:")
                .with_send("${username}\n"),
        );

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(result.success, "dialog failed: {:?}", result.error);
        assert_eq!(handle.take_input_str(), "admin\n");
    }

    /// Execution begins at the first step. It used to begin at the first
    /// *named* one, so a dialog whose opening steps were unnamed skipped them.
    #[tokio::test]
    async fn execution_begins_at_the_first_step() {
        let (mut session, handle) = session_with("one two three ");

        let dialog = Dialog::named("mixed")
            .step(DialogStep::expect("one").then_send("a\n"))
            .step(DialogStep::expect("two").then_send("b\n"))
            .step(
                DialogStep::new("named")
                    .with_expect("three")
                    .with_send("c\n"),
            );

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(result.success, "dialog failed: {:?}", result.error);
        assert_eq!(
            result.steps.len(),
            3,
            "the unnamed steps must not be skipped"
        );
        assert_eq!(handle.take_input_str(), "a\nb\nc\n");
    }

    /// Same defect on the entry point: it used to fall back to step 0 and run
    /// a different dialog than the caller asked for.
    #[tokio::test]
    async fn unknown_entry_point_fails() {
        let (mut session, handle) = session_with("hello ");

        let dialog = Dialog::named("d")
            .step(
                DialogStep::new("first")
                    .with_expect("hello")
                    .with_send("a\n"),
            )
            .entry_point("nowhere");

        let result = short_executor()
            .execute(&mut session, &dialog)
            .await
            .expect("dialog runs");

        assert!(!result.success, "unknown entry point must not succeed");
        assert!(result.steps.is_empty(), "no step should have run");
        assert_eq!(handle.take_input_str(), "", "nothing should have been sent");
    }
}
