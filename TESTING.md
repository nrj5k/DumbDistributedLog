# AutoQueues Testing Guidelines

This document serves as the official testing guide for AutoQueues development. All team members should follow these principles and practices to ensure consistent, high-quality tests throughout the codebase.

## Test Philosophy & Principles

### Core Testing Philosophy

Tests in AutoQueues follow the principle that they should:
- **Verify behavior, not implementation details**
- **Be readable and serve as documentation**
- **Run quickly and reliably**
- **Fail clearly with actionable error messages**
- **Cover edge cases and error conditions**
- **Be deterministic (no flaky tests)**

We prioritize testing the contract/interface of our components over internal implementation details.

### Test Types in AutoQueues

1. **Unit Tests**: Test individual functions and modules in isolation
2. **Integration Tests**: Test interactions between multiple components
3. **Property-Based Tests**: Test mathematical properties and invariants
4. **Performance Tests**: Benchmark critical paths
5. **End-to-End Tests**: Full system behavior validation

## Test Structure & Organization

### File Organization

```
src/
  module.rs        # Source code
  module/test.rs   # Unit tests (preferred for smaller modules)
tests/
  integration/     # Integration tests
  benchmarks/      # Benchmark tests
```

For small modules, use inline tests with `#[cfg(test)] mod tests`. For larger modules with extensive testing, consider separate test files.

### Test Naming Convention

Follow this pattern: `test_[function_name]_[scenario]`

Examples:
- `test_queue_creation_with_valid_config`
- `test_queue_push_pop_roundtrip`
- `test_queue_overflow_behavior`
- `test_queue_concurrent_access`

### AAA Pattern (Arrange, Act, Assert)

Structure all tests following the AAA pattern:

```rust
#[test]
fn test_queue_push_pop_roundtrip() {
    // Arrange
    let mut queue = Queue::new();
    let test_data = "test_value";

    // Act
    queue.push(test_data).await.unwrap();
    let result = queue.pop().await.unwrap();

    // Assert
    assert_eq!(result, test_data);
}
```

## Assertions & Verification Guidelines

### Clear Assertion Messages

Always provide context in assertion messages:

```rust
// Good - provides context about what failed
assert_eq!(
    result.len(), 
    expected_length, 
    "Queue should contain exactly {} items after pop operation", 
    expected_length
);

// Bad - no context
assert_eq!(result.len(), expected_length);
```

### Use Appropriate Assertion Macros

| Situation | Recommended Macro |
|-----------|-------------------|
| Value equality | `assert_eq!` |
| Boolean conditions | `assert!` |
| Error expectation | `assert!(matches!(result, Err(QueueError::InvalidData)))` |
| Floating point comparison | `assert!((a - b).abs() < epsilon)` |

### Error Testing

Test for expected errors explicitly:

```rust
#[tokio::test]
async fn test_invalid_queue_creation_fails() {
    let config = InvalidQueueConfig::new();
    
    let result = Queue::new(config).await;
    
    assert!(matches!(result, Err(QueueError::InvalidConfiguration)));
}
```

## What to Test and What NOT to Test

### DO Test

1. **Public API contracts**
   - All public methods with valid and invalid inputs
   - Edge cases (empty, boundary values, maximum capacity)
   - Error conditions and recovery paths
   - Async behavior and timing considerations

2. **Business Logic**
   - Expression evaluation correctness
   - Queue processing mechanics
   - Pub/sub topic matching logic
   - Metric calculation accuracy

3. **Integration Points**
   - Redis-like API endpoints
   - Network communication handlers
   - Configuration parsing and validation
   - External dependencies with mocked interfaces

### DON'T Test

1. **Implementation Details**
   - Private helper functions (unless critical)
   - Internal data structures unless part of API
   - Third-party library functionality
   - Standard library behaviors

2. **Trivial Getters/Setters**
   - Basic accessor methods without logic
   - Simple field assignments

3. **Generated Code**
   - Derive macro outputs
   - Boilerplate serialization code
   - Auto-implemented trait functions

## Test Anti-patterns to Avoid

### 1. Over-Mocking

Avoid mocking everything. Only mock external dependencies (filesystem, network, etc.).

```rust
// BAD - Over-mocked
#[test]
fn test_queue_processing() {
    let mock_storage = MockStorage::new();
    let mock_processor = MockProcessor::new();
    let mock_validator = MockValidator::new();
    // ... lots of setup
    
    // This test verifies nothing useful
}

// GOOD - Test real behavior with minimal mocking
#[tokio::test]
async fn test_queue_processing_integration() {
    let queue = Queue::new(test_config());
    queue.push(test_data()).await.unwrap();
    
    let result = queue.process().await;
    assert!(result.is_ok());
}
```

### 2. Brittle Tests

Avoid tests that break when implementation changes slightly:

```rust
// BAD - Brittle test dependent on implementation
#[test]
fn test_internal_state() {
    // Checking private fields that aren't part of public API
    assert_eq!(queue.internal_counter, 5);
}

// GOOD - Test observable behavior
#[tokio::test]
async fn test_queue_capacity_limit() {
    let queue = Queue::with_capacity(5);
    // Fill queue to capacity
    for i in 0..5 {
        queue.push(format!("item_{}", i)).await.unwrap();
    }
    
    // Next push should fail
    let result = queue.push("overflow_item".to_string()).await;
    assert!(matches!(result, Err(QueueError::QueueFull)));
}
```

### 3. Timing-Dependent Tests

Avoid tests that rely on sleep or precise timing:

```rust
// BAD - Timing-dependent
#[tokio::test]
async fn test_async_operation() {
    let handle = spawn_async_operation();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(handle.is_finished()); // May fail intermittently
}

// GOOD - Event-driven verification
#[tokio::test]
async fn test_async_operation_completion() {
    let (tx, rx) = oneshot::channel();
    
    spawn_async_operation(move || {
        // Do work...
        tx.send(()).unwrap();
    });
    
    // Wait for completion signal
    rx.await.unwrap();
    // Now we know it's done
}
```

## Integration Test Guidelines

### Real Component Testing

Prefer testing with real components over mocks where possible:

```rust
// Better approach for integration tests
#[tokio::test]
async fn test_pubsub_end_to_end() {
    // Set up real broker
    let broker = PubSubBroker::new(1000);
    
    // Create publisher and subscriber
    let publisher = Publisher::new(broker.clone());
    let subscriber = broker.subscribe("test.topic").await.unwrap();
    
    // Publish and verify
    publisher.publish("test.topic", "hello").await.unwrap();
    
    let received = subscriber.recv().await.unwrap();
    assert_eq!(received.data, "hello");
}
```

### Resource Management

Always clean up test resources:

```rust
#[tokio::test]
async fn test_with_temporary_files() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    // Test using temporary database
    let db = Database::open(&db_path).await.unwrap();
    // ... test operations ...
    
    // Cleanup happens automatically when temp_dir goes out of scope
}
```

## Example Templates

### Good Unit Test Template

```rust
#[tokio::test]
async fn test_queue_push_valid_data_success() {
    // Arrange
    let mut queue = Queue::new("test_queue");
    let test_payload = serde_json::json!({"cpu": 80.5, "memory": 60.2});
    
    // Act
    let result = queue.push(test_payload).await;
    
    // Assert
    assert!(
        result.is_ok(), 
        "Push operation should succeed with valid JSON data"
    );
    assert_eq!(
        queue.len().await, 
        1, 
        "Queue length should be 1 after successful push"
    );
}
```

### Good Integration Test Template

```rust
#[tokio::test]
async fn test_autoqueues_server_mode_push_subscribe_flow() {
    // Arrange
    let server = AutoQueuesServer::minimal()
        .port(0) // Let OS choose port
        .push_topic("metrics.cpu")
        .subscribe_topic("alerts.*")
        .build()
        .await
        .expect("Failed to build server");
    
    let port = server.local_addr().port();
    let client = reqwest::Client::new();
    
    // Start server in background
    tokio::spawn(async move {
        server.run().await.expect("Server failed");
    });
    
    // Allow server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Act - Push data
    let payload = json!({"timestamp": "2024-01-01T00:00:00Z", "value": 85.5});
    let response = client
        .post(&format!("http://localhost:{}/push/metrics.cpu", port))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");
    
    // Assert
    assert_eq!(response.status(), 200);
    
    // TODO: Verify subscription receives data
    // This would involve setting up a websocket client or similar
}
```

### Bad Test Examples (What to Avoid)

```rust
// BAD - Tests implementation details
#[test]
fn test_internal_buffer_allocation() {
    let queue = Queue::new();
    assert_eq!(queue.buffer.capacity(), 100); // Implementation detail
    assert_eq!(queue.internal_state.flag, false); // Private state
}

// BAD - No clear purpose
#[test]
fn test_something() { // Unclear name
    let q = Queue::new();
    q.push("data").await.unwrap();
    // No assertions - what is being tested?
}

// BAD - Overcomplicated setup
#[test]
fn test_with_excessive_mocking() {
    let mock_network = MockNetwork::new();
    mock_network.expect_connect().times(1).returning(|| Ok(()));
    mock_network.expect_send().times(3).returning(|_| Ok(()));
    mock_network.expect_disconnect().times(1).returning(|| Ok(()));
    
    let mock_parser = MockParser::new();
    mock_parser.expect_parse().times(1).returning(|| Ok(parsed_data()));
    
    let mock_validator = MockValidator::new();
    mock_validator.expect_validate().times(1).returning(|| Ok(()));
    
    // Actual test logic is buried in setup
    // Test isn't verifying meaningful behavior
}
```

## Test Review Checklist

Before committing tests, ensure they meet these criteria:

### Essential Requirements
- [ ] Test name clearly describes what is being tested
- [ ] Test follows AAA pattern (Arrange, Act, Assert)
- [ ] Test verifies externally observable behavior, not implementation details
- [ ] Test includes proper error handling verification
- [ ] Test has meaningful assertions with descriptive failure messages
- [ ] Test is deterministic (no random failures)

### Quality Attributes
- [ ] Test runs quickly (< 1 second typically)
- [ ] Test is focused (tests one thing well)
- [ ] Test cleans up resources properly
- [ ] Test handles edge cases appropriately
- [ ] Test failure messages are actionable
- [ ] Test is maintainable and readable

### Coverage Considerations
- [ ] Happy path scenarios covered
- [ ] Error conditions tested
- [ ] Boundary values checked
- [ ] Concurrent access patterns considered (for async components)
- [ ] Configuration variations tested (where applicable)

### Integration Specific
- [ ] Real dependencies used when practical
- [ ] External resources properly managed
- [ ] Network/file system operations cleaned up
- [ ] End-to-end flows validated appropriately

## Performance and Maintenance

### Test Speed Optimization

1. Reuse expensive setup in multiple tests:
```rust
struct TestContext {
    queue: Queue,
    broker: PubSubBroker,
}

impl TestContext {
    fn new() -> Self {
        // Expensive setup once
        Self {
            queue: Queue::new(test_config()),
            broker: PubSubBroker::new(1000),
        }
    }
}

#[tokio::test]
async fn test_queue_publishing() {
    let ctx = TestContext::new();
    // Test using shared context
}
```

2. Use appropriate test attributes:
```rust
// For expensive tests
#[ignore] // Run with `cargo test -- --ignored`
#[tokio::test]
async fn test_large_dataset_processing() {
    // Test that takes significant time/resources
}

// For quick unit tests
#[tokio::test]
async fn test_simple_validation() {
    // Fast unit test
}
```

## CI/CD Testing Guidelines

Tests should be organized to support efficient CI/CD pipelines:

1. **Fast unit tests** (under 5 seconds): Run on every commit
2. **Integration tests** (under 30 seconds): Run on pull requests
3. **Long-running tests** (over 30 seconds): Use `#[ignore]` attribute
4. **Flaky tests**: Fix or quarantine immediately

All tests should be runnable with:
```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test "*"

# Run ignored tests
cargo test -- --ignored

# Run all tests
cargo test
```

---
*Last updated: February 4, 2026*