//! Integration tests for synchronous API.

use rust_expect::block_on;

#[test]
fn block_on_simple_future() {
    let result = block_on(async { 42 });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn block_on_async_operation() {
    let result = block_on(async {
        let x = 1;
        let y = 2;
        format!("sum: {}", x + y)
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sum: 3");
}

#[test]
fn block_on_nested_async() {
    async fn inner() -> i32 {
        100
    }

    let result = block_on(async { inner().await + 1 });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 101);
}

// Note: SyncSession tests require spawning actual processes,
// which is done in spawn_integration tests
