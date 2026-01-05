//! Mock scenarios for testing expect scripts.
//!
//! Scenarios define a sequence of expected patterns and responses
//! for simulating interactive sessions.

use std::time::Duration;

use super::event::{EventTimeline, MockEvent};

/// A step in a mock scenario.
#[derive(Debug, Clone)]
pub struct ScenarioStep {
    /// Pattern to wait for (if any).
    pub expect: Option<String>,
    /// Response to send.
    pub response: Option<String>,
    /// Delay before response.
    pub delay: Duration,
    /// Optional error to inject.
    pub error: Option<String>,
}

impl Default for ScenarioStep {
    fn default() -> Self {
        Self {
            expect: None,
            response: None,
            delay: Duration::ZERO,
            error: None,
        }
    }
}

impl ScenarioStep {
    /// Create a new scenario step.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the pattern to expect.
    #[must_use]
    pub fn expect(mut self, pattern: impl Into<String>) -> Self {
        self.expect = Some(pattern.into());
        self
    }

    /// Set the response to send.
    #[must_use]
    pub fn respond(mut self, response: impl Into<String>) -> Self {
        self.response = Some(response.into());
        self
    }

    /// Set the delay before response.
    #[must_use]
    pub const fn delay(mut self, duration: Duration) -> Self {
        self.delay = duration;
        self
    }

    /// Set the delay in milliseconds.
    #[must_use]
    pub const fn delay_ms(mut self, ms: u64) -> Self {
        self.delay = Duration::from_millis(ms);
        self
    }

    /// Set an error to inject.
    #[must_use]
    pub fn error(mut self, msg: impl Into<String>) -> Self {
        self.error = Some(msg.into());
        self
    }
}

/// A complete mock scenario.
#[derive(Debug, Clone, Default)]
pub struct Scenario {
    /// Name of the scenario.
    name: String,
    /// Description of the scenario.
    description: String,
    /// Steps in the scenario.
    steps: Vec<ScenarioStep>,
    /// Initial output before any interaction.
    initial_output: Option<String>,
    /// Exit code for the scenario.
    exit_code: Option<i32>,
}

impl Scenario {
    /// Create a new scenario.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
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

    /// Set initial output.
    #[must_use]
    pub fn initial_output(mut self, output: impl Into<String>) -> Self {
        self.initial_output = Some(output.into());
        self
    }

    /// Add a step to the scenario.
    #[must_use]
    pub fn step(mut self, step: ScenarioStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Add an expect-respond pair.
    #[must_use]
    pub fn expect_respond(self, pattern: impl Into<String>, response: impl Into<String>) -> Self {
        self.step(ScenarioStep::new().expect(pattern).respond(response))
    }

    /// Set the exit code.
    #[must_use]
    pub const fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Get the scenario name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the scenario description.
    #[must_use]
    pub fn get_description(&self) -> &str {
        &self.description
    }

    /// Get the steps.
    #[must_use]
    pub fn steps(&self) -> &[ScenarioStep] {
        &self.steps
    }

    /// Convert to an event timeline.
    #[must_use]
    pub fn to_timeline(&self) -> EventTimeline {
        let mut events = Vec::new();

        // Add initial output
        if let Some(output) = &self.initial_output {
            events.push(MockEvent::output_str(output));
        }

        // Add steps
        for step in &self.steps {
            // Add delay if specified
            if !step.delay.is_zero() {
                events.push(MockEvent::delay(step.delay));
            }

            // Add error if specified
            if let Some(error) = &step.error {
                events.push(MockEvent::error(error.clone()));
            }

            // Add response as output
            if let Some(response) = &step.response {
                events.push(MockEvent::output_str(response));
            }
        }

        // Add exit
        if let Some(code) = self.exit_code {
            events.push(MockEvent::exit(code));
        }

        EventTimeline::from_events(events)
    }
}

/// Builder for creating scenarios fluently.
pub struct ScenarioBuilder {
    scenario: Scenario,
}

impl ScenarioBuilder {
    /// Create a new scenario builder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            scenario: Scenario::new(name),
        }
    }

    /// Set the description.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.scenario = self.scenario.description(desc);
        self
    }

    /// Add initial output.
    #[must_use]
    pub fn initial_output(mut self, output: impl Into<String>) -> Self {
        self.scenario = self.scenario.initial_output(output);
        self
    }

    /// Add a login prompt step.
    #[must_use]
    pub fn login_prompt(self) -> Self {
        self.step(ScenarioStep::new().respond("login: "))
    }

    /// Add a password prompt step.
    #[must_use]
    pub fn password_prompt(self) -> Self {
        self.step(ScenarioStep::new().respond("Password: "))
    }

    /// Add a shell prompt step.
    #[must_use]
    pub fn shell_prompt(self, prompt: impl Into<String>) -> Self {
        self.step(ScenarioStep::new().respond(prompt))
    }

    /// Add a custom step.
    #[must_use]
    pub fn step(mut self, step: ScenarioStep) -> Self {
        self.scenario = self.scenario.step(step);
        self
    }

    /// Add an expect-respond pair.
    #[must_use]
    pub fn expect_respond(
        mut self,
        pattern: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        self.scenario = self.scenario.expect_respond(pattern, response);
        self
    }

    /// Set the exit code.
    #[must_use]
    pub fn exit_code(mut self, code: i32) -> Self {
        self.scenario = self.scenario.exit_code(code);
        self
    }

    /// Build the scenario.
    #[must_use]
    pub fn build(self) -> Scenario {
        self.scenario
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_basic() {
        let scenario = Scenario::new("test")
            .initial_output("Welcome\n")
            .expect_respond("login:", "user\n")
            .expect_respond("password:", "pass\n")
            .exit_code(0);

        assert_eq!(scenario.name(), "test");
        assert_eq!(scenario.steps().len(), 2);
    }

    #[test]
    fn scenario_to_timeline() {
        let scenario = Scenario::new("test").initial_output("Hello\n").exit_code(0);

        let timeline = scenario.to_timeline();
        assert_eq!(timeline.events().len(), 2);
        assert!(timeline.events()[0].is_output());
        assert!(timeline.events()[1].is_exit());
    }

    #[test]
    fn scenario_builder() {
        let scenario = ScenarioBuilder::new("login")
            .description("A login scenario")
            .login_prompt()
            .password_prompt()
            .shell_prompt("$ ")
            .exit_code(0)
            .build();

        assert_eq!(scenario.name(), "login");
        assert_eq!(scenario.steps().len(), 3);
    }
}
