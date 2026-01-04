//! Test utilities for rust-expect.
//!
//! This module provides testing utilities, mocks, and helpers for
//! writing tests against expect-based terminal automation.

mod assertions;
mod builders;
mod fake_pty;
mod fixtures;
mod mock_session;

pub use assertions::{OutputAssertions, assert_output_contains, assert_output_matches};
pub use builders::{ExpectTestBuilder, SessionTestBuilder};
pub use fake_pty::{FakePty, FakePtyPair};
pub use fixtures::{Fixtures, TestFixture, find_fixtures_dir};
pub use mock_session::{RecordedInteraction, TestSession, TestSessionBuilder};
