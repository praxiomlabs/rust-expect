//! Mock session support for testing.
//!
//! This module provides mock implementations for testing expect scripts
//! without spawning real processes. It includes:
//!
//! - [`MockTransport`]: A mock async transport for testing
//! - [`MockSession`]: A complete mock session
//! - [`Scenario`]: Pre-defined interaction scenarios
//! - Built-in scenarios for common use cases
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_expect::mock::{MockSession, Scenario, ScenarioStep};
//!
//! // Create a mock session with a login scenario
//! let scenario = Scenario::new("test")
//!     .initial_output("Login: ")
//!     .expect_respond("username", "Password: ")
//!     .expect_respond("password", "Welcome!\n$ ");
//!
//! let session = MockSession::from_scenario(&scenario);
//! ```

pub mod builtin;
pub mod event;
pub mod scenario;
pub mod session;

pub use builtin::*;
pub use event::{EventTimeline, MockEvent};
pub use scenario::{Scenario, ScenarioBuilder, ScenarioStep};
pub use session::{MockSession, MockTransport};

/// Create a simple mock transport with pre-queued output.
///
/// # Example
///
/// ```rust
/// use rust_expect::mock::simple_mock;
///
/// let transport = simple_mock("Hello, World!\n");
/// ```
#[must_use]
pub fn simple_mock(output: &str) -> MockTransport {
    let transport = MockTransport::new();
    transport.queue_output_str(output);
    transport
}

/// Create a mock transport that simulates a login prompt.
#[must_use]
pub fn login_mock() -> MockTransport {
    MockTransport::from_scenario(&builtin::login_scenario("user", "pass"))
}

/// Create a mock transport that simulates a shell prompt.
#[must_use]
pub fn shell_mock(prompt: &str) -> MockTransport {
    MockTransport::from_scenario(&builtin::shell_scenario(prompt))
}

/// Builder for creating mock transports fluently.
pub struct MockBuilder {
    events: Vec<MockEvent>,
}

impl MockBuilder {
    /// Create a new mock builder.
    #[must_use]
    pub const fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add output to the mock.
    #[must_use]
    pub fn output(mut self, data: &str) -> Self {
        self.events.push(MockEvent::output_str(data));
        self
    }

    /// Add a delay.
    #[must_use]
    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.events.push(MockEvent::delay_ms(ms));
        self
    }

    /// Signal EOF.
    #[must_use]
    pub fn eof(mut self) -> Self {
        self.events.push(MockEvent::eof());
        self
    }

    /// Signal exit.
    #[must_use]
    pub fn exit(mut self, code: i32) -> Self {
        self.events.push(MockEvent::exit(code));
        self
    }

    /// Build the mock transport.
    #[must_use]
    pub fn build(self) -> MockTransport {
        MockTransport::from_timeline(EventTimeline::from_events(self.events))
    }
}

impl Default for MockBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_mock_creates_transport() {
        let transport = simple_mock("test");
        assert!(!transport.is_eof());
    }

    #[test]
    fn mock_builder_works() {
        let transport = MockBuilder::new()
            .output("Hello\n")
            .delay_ms(100)
            .output("World\n")
            .exit(0)
            .build();

        assert!(!transport.is_eof());
    }
}
