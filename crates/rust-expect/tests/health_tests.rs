//! Integration tests for health checking.

use rust_expect::health::{HealthCheckConfig, HealthCheckResult, HealthChecker};
use rust_expect::HealthStatus;
use std::time::Duration;

#[test]
fn health_status_variants() {
    let _ = HealthStatus::Healthy;
    let _ = HealthStatus::Degraded;
    let _ = HealthStatus::Unhealthy;
    let _ = HealthStatus::Unknown;
}

#[test]
fn health_status_display() {
    assert!(!format!("{:?}", HealthStatus::Healthy).is_empty());
    assert!(!format!("{:?}", HealthStatus::Degraded).is_empty());
    assert!(!format!("{:?}", HealthStatus::Unhealthy).is_empty());
}

#[test]
fn health_status_is_healthy() {
    assert!(HealthStatus::Healthy.is_healthy());
    assert!(!HealthStatus::Degraded.is_healthy());
    assert!(!HealthStatus::Unhealthy.is_healthy());
}

#[test]
fn health_status_is_operational() {
    assert!(HealthStatus::Healthy.is_operational());
    assert!(HealthStatus::Degraded.is_operational());
    assert!(!HealthStatus::Unhealthy.is_operational());
}

#[test]
fn health_status_equality() {
    assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
    assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
}

#[test]
fn health_check_config_default() {
    let config = HealthCheckConfig::default();
    assert_eq!(config.interval, Duration::from_secs(30));
    assert_eq!(config.timeout, Duration::from_secs(5));
    assert_eq!(config.failure_threshold, 3);
    assert_eq!(config.success_threshold, 1);
}

#[test]
fn health_check_config_builder() {
    let config = HealthCheckConfig::new()
        .with_interval(Duration::from_secs(10))
        .with_timeout(Duration::from_secs(2))
        .with_failure_threshold(5)
        .with_success_threshold(2);

    assert_eq!(config.interval, Duration::from_secs(10));
    assert_eq!(config.timeout, Duration::from_secs(2));
    assert_eq!(config.failure_threshold, 5);
    assert_eq!(config.success_threshold, 2);
}

#[test]
fn health_checker_new() {
    let config = HealthCheckConfig::default();
    let checker = HealthChecker::new(config);
    assert_eq!(checker.status(), HealthStatus::Unknown);
}

#[test]
fn health_checker_record_success() {
    let config = HealthCheckConfig::default();
    let mut checker = HealthChecker::new(config);

    checker.record_success();
    assert_eq!(checker.status(), HealthStatus::Healthy);
}

#[test]
fn health_checker_record_failure() {
    let config = HealthCheckConfig {
        failure_threshold: 2,
        ..Default::default()
    };
    let mut checker = HealthChecker::new(config);

    checker.record_failure("test failure");
    assert_eq!(checker.status(), HealthStatus::Degraded);

    checker.record_failure("second failure");
    assert_eq!(checker.status(), HealthStatus::Unhealthy);
}

#[test]
fn health_check_result_healthy() {
    let result = HealthCheckResult::healthy();
    assert_eq!(result.status, HealthStatus::Healthy);
    assert!(result.message.is_none());
}

#[test]
fn health_check_result_unhealthy() {
    let result = HealthCheckResult::unhealthy("Something went wrong");
    assert_eq!(result.status, HealthStatus::Unhealthy);
    assert_eq!(result.message, Some("Something went wrong".to_string()));
}

#[test]
fn health_check_result_degraded() {
    let result = HealthCheckResult::degraded("Partially working");
    assert_eq!(result.status, HealthStatus::Degraded);
    assert_eq!(result.message, Some("Partially working".to_string()));
}
