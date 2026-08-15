//! Integration tests for rust-expect.
//!
//! Note: Main integration tests (spawn, expect, mock) are in separate files
//! at the tests/ directory level for better discovery.
//!
//! Declared by `tests/integration_tests.rs`. Without that root, cargo never
//! compiles this directory: the files sat here for a long time neither built
//! nor run, drifting out of date against the API they were testing.

mod dialog_tests;
mod session_tests;
