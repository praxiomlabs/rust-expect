//! Platform-specific tests.
//!
//! This module contains tests that are specific to Unix or Windows platforms.

#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;
