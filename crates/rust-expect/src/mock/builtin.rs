//! Built-in mock scenarios for common use cases.
//!
//! This module provides pre-configured scenarios for common
//! terminal interactions like login prompts, shell sessions, etc.

use super::event::{EventTimeline, MockEvent};
use super::scenario::{Scenario, ScenarioBuilder, ScenarioStep};
use std::time::Duration;

/// Create a login scenario.
///
/// Simulates a typical login prompt sequence.
#[must_use]
pub fn login_scenario(username: &str, password: &str) -> Scenario {
    ScenarioBuilder::new("login")
        .description("Standard login sequence")
        .initial_output("Welcome to the system\n\n")
        .step(ScenarioStep::new().respond("login: "))
        .step(ScenarioStep::new().expect(username).respond("\nPassword: "))
        .step(ScenarioStep::new().expect(password).respond("\nLast login: Mon Jan 1 00:00:00\n$ "))
        .exit_code(0)
        .build()
}

/// Create an SSH connection scenario.
///
/// Simulates an SSH connection with host key verification.
#[must_use]
pub fn ssh_scenario(host: &str) -> Scenario {
    let banner = format!(
        "The authenticity of host '{host}' can't be established.\n\
         RSA key fingerprint is SHA256:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX.\n\
         Are you sure you want to continue connecting (yes/no)? "
    );

    ScenarioBuilder::new("ssh")
        .description("SSH connection with host key verification")
        .step(ScenarioStep::new().respond(&banner))
        .step(ScenarioStep::new().expect("yes").respond("\nWarning: Permanently added '{}' to the list of known hosts.\n"))
        .step(ScenarioStep::new().respond("Password: "))
        .step(ScenarioStep::new().delay_ms(100).respond("\nLast login: Mon Jan 1 00:00:00\n$ "))
        .exit_code(0)
        .build()
}

/// Create a shell session scenario.
///
/// Simulates a simple shell session with a prompt.
#[must_use]
pub fn shell_scenario(prompt: &str) -> Scenario {
    ScenarioBuilder::new("shell")
        .description("Interactive shell session")
        .initial_output(prompt)
        .exit_code(0)
        .build()
}

/// Create a command execution scenario.
///
/// Simulates running a command and getting output.
#[must_use]
pub fn command_scenario(command: &str, output: &str, exit_code: i32) -> Scenario {
    ScenarioBuilder::new("command")
        .description("Command execution")
        .step(ScenarioStep::new().expect(command).delay_ms(50).respond(output))
        .exit_code(exit_code)
        .build()
}

/// Create a sudo scenario.
///
/// Simulates a sudo password prompt.
#[must_use]
pub fn sudo_scenario(command: &str) -> Scenario {
    ScenarioBuilder::new("sudo")
        .description("Sudo password prompt")
        .step(ScenarioStep::new().respond("[sudo] password: "))
        .step(ScenarioStep::new().delay_ms(100).respond(format!("\nExecuting: {command}\n")))
        .exit_code(0)
        .build()
}

/// Create a menu selection scenario.
///
/// Simulates a menu with numbered options.
#[must_use]
pub fn menu_scenario(options: &[&str]) -> Scenario {
    let mut menu = String::from("Please select an option:\n");
    for (i, option) in options.iter().enumerate() {
        menu.push_str(&format!("  {}. {}\n", i + 1, option));
    }
    menu.push_str("Enter your choice: ");

    ScenarioBuilder::new("menu")
        .description("Menu selection")
        .step(ScenarioStep::new().respond(&menu))
        .exit_code(0)
        .build()
}

/// Create a yes/no prompt scenario.
#[must_use]
pub fn yes_no_scenario(question: &str) -> Scenario {
    ScenarioBuilder::new("yesno")
        .description("Yes/No prompt")
        .step(ScenarioStep::new().respond(format!("{question} [y/n]: ")))
        .exit_code(0)
        .build()
}

/// Create a file transfer scenario.
///
/// Simulates an SCP-like file transfer with progress.
#[must_use]
pub fn file_transfer_scenario(filename: &str, size_kb: usize) -> Scenario {
    let mut builder = ScenarioBuilder::new("transfer")
        .description("File transfer with progress")
        .initial_output(format!("Transferring {filename}...\n"));

    // Add progress updates
    let steps = 10;
    let chunk_size = size_kb / steps;
    for i in 1..=steps {
        let progress = i * 10;
        let transferred = chunk_size * i;
        builder = builder.step(
            ScenarioStep::new()
                .delay_ms(100)
                .respond(format!("\r[{}{}] {}% ({}/{}KB)", 
                    "=".repeat(i),
                    " ".repeat(steps - i),
                    progress,
                    transferred,
                    size_kb
                ))
        );
    }

    builder
        .step(ScenarioStep::new().respond(format!("\n{filename} transferred successfully.\n")))
        .exit_code(0)
        .build()
}

/// Create an error scenario.
///
/// Simulates an error message and non-zero exit.
#[must_use]
pub fn error_scenario(error_message: &str, exit_code: i32) -> Scenario {
    ScenarioBuilder::new("error")
        .description("Error scenario")
        .step(ScenarioStep::new().respond(format!("Error: {error_message}\n")))
        .exit_code(exit_code)
        .build()
}

/// Create a timeout scenario.
///
/// Simulates a delayed response.
#[must_use]
pub fn timeout_scenario(delay: Duration) -> Scenario {
    ScenarioBuilder::new("timeout")
        .description("Delayed response scenario")
        .step(ScenarioStep::new().delay(delay))
        .exit_code(0)
        .build()
}

/// Create an interactive prompt scenario.
///
/// Simulates a series of prompts and responses.
#[must_use]
pub fn interactive_prompts(prompts: &[(&str, &str)]) -> Scenario {
    let mut builder = ScenarioBuilder::new("interactive")
        .description("Interactive prompts");

    for (prompt, response) in prompts {
        builder = builder.step(
            ScenarioStep::new()
                .respond(*prompt)
        );
        builder = builder.step(
            ScenarioStep::new()
                .expect(*response)
        );
    }

    builder.exit_code(0).build()
}

/// Generate an event timeline for a bash session.
#[must_use]
pub fn bash_session() -> EventTimeline {
    EventTimeline::from_events(vec![
        MockEvent::output_str("bash-5.0$ "),
        MockEvent::Delay(Duration::from_millis(10)),
    ])
}

/// Generate an event timeline for a Python REPL.
#[must_use]
pub fn python_repl() -> EventTimeline {
    EventTimeline::from_events(vec![
        MockEvent::output_str("Python 3.10.0 (default, Jan 1 2024, 00:00:00)\n"),
        MockEvent::output_str("[GCC 9.3.0] on linux\n"),
        MockEvent::output_str("Type \"help\", \"copyright\", \"credits\" or \"license\" for more information.\n"),
        MockEvent::output_str(">>> "),
    ])
}

/// Generate an event timeline for a Node.js REPL.
#[must_use]
pub fn node_repl() -> EventTimeline {
    EventTimeline::from_events(vec![
        MockEvent::output_str("Welcome to Node.js v18.0.0.\n"),
        MockEvent::output_str("Type \".help\" for more information.\n"),
        MockEvent::output_str("> "),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_scenario_creates_valid_scenario() {
        let scenario = login_scenario("user", "pass");
        assert_eq!(scenario.name(), "login");
        assert!(!scenario.steps().is_empty());
    }

    #[test]
    fn menu_scenario_creates_valid_menu() {
        let scenario = menu_scenario(&["Option A", "Option B", "Option C"]);
        let timeline = scenario.to_timeline();
        assert!(!timeline.events().is_empty());
    }

    #[test]
    fn file_transfer_has_progress() {
        let scenario = file_transfer_scenario("test.txt", 1000);
        let timeline = scenario.to_timeline();
        assert!(timeline.events().len() > 10); // Should have multiple progress updates
    }
}
