//! Mock testing example.
//!
//! This example demonstrates using the mock module for testing
//! expect scripts without spawning real processes.
//!
//! Run with: `cargo run --example mock_testing --features mock`

#[cfg(feature = "mock")]
use rust_expect::mock::{
    MockBuilder, MockSession, MockTransport, Scenario, login_mock, shell_mock, simple_mock,
};

fn main() {
    println!("rust-expect Mock Testing Example");
    println!("=================================\n");

    #[cfg(not(feature = "mock"))]
    {
        println!("This example requires the 'mock' feature.");
        println!("Run with: cargo run --example mock_testing --features mock");
        return;
    }

    #[cfg(feature = "mock")]
    run_examples();
}

#[cfg(feature = "mock")]
fn run_examples() {
    // Example 1: Simple mock transport
    println!("1. Simple mock transport...");

    let transport = simple_mock("Hello, World!\n");
    println!("   Created mock with pre-queued output");
    println!("   EOF status: {}", transport.is_eof());

    // Example 2: Mock builder pattern
    println!("\n2. Using MockBuilder...");

    let transport = MockBuilder::new()
        .output("Welcome to the system\n")
        .delay_ms(100)
        .output("Login: ")
        .delay_ms(50)
        .output("Password: ")
        .delay_ms(100)
        .output("Login successful!\n$ ")
        .exit(0)
        .build();

    println!("   Built mock with multiple outputs and delays");

    // Example 3: Pre-built scenarios
    println!("\n3. Pre-built scenarios...");

    // Login scenario
    let login = login_mock();
    println!("   Created login scenario mock");

    // Shell scenario
    let shell = shell_mock("$ ");
    println!("   Created shell scenario mock");

    // Example 4: Custom scenarios
    println!("\n4. Custom scenarios...");

    let scenario = Scenario::new("ftp-session")
        .initial_output("220 FTP Server Ready\n")
        .expect_respond("USER anonymous", "331 Password required\n")
        .expect_respond("PASS guest@", "230 User logged in\n")
        .expect_respond("PWD", "257 \"/\" is current directory\n")
        .expect_respond("QUIT", "221 Goodbye\n");

    println!("   Created FTP session scenario: {}", scenario.name());

    let ftp_mock = MockTransport::from_scenario(&scenario);
    println!("   Mock transport ready for testing");

    // Example 5: Mock session for expect testing
    println!("\n5. Mock session for testing...");

    let scenario = Scenario::new("ssh-login")
        .initial_output("user@host's password: ")
        .expect_respond("secret123", "\nWelcome to Ubuntu 22.04\n$ ")
        .expect_respond("whoami", "user\n$ ")
        .expect_respond("exit", "logout\n");

    let session = MockSession::from_scenario(&scenario);
    println!("   Created mock SSH session");
    println!("   Ready for expect script testing");

    // Example 6: Testing with assertions
    println!("\n6. Mock testing workflow...");

    // This demonstrates how you'd use mocks in tests
    let scenario = Scenario::new("test-case")
        .initial_output("Enter value: ")
        .expect_respond("42", "Answer accepted\nResult: 84\n");

    let _mock = MockSession::from_scenario(&scenario);

    // In a real test, you would:
    // 1. Create the mock with expected behavior
    // 2. Run your automation code against it
    // 3. Assert the results

    println!("   Mock testing enables:");
    println!("   - Fast, deterministic tests");
    println!("   - No real process spawning");
    println!("   - Precise control over timing");
    println!("   - Simulating error conditions");

    // Example 7: Timeline-based mock
    println!("\n7. Event timeline mock...");

    let transport = MockBuilder::new()
        .output("Connecting...")
        .delay_ms(500)
        .output(" connected!\n")
        .output("Ready> ")
        .delay_ms(100)
        .output("Processing...")
        .delay_ms(1000)
        .output(" done!\n")
        .eof()
        .build();

    println!("   Created timeline with realistic delays");
    println!("   Useful for testing timeout handling");

    println!("\nMock testing examples completed successfully!");
}
