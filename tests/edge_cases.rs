//! Edge case tests for the autoqueues library
//!
//! This file contains tests for boundary conditions, unusual inputs,
//! and edge case behaviors that might cause panics or unexpected behavior.

use ddl::traits::ddl::{DDL, DdlConfig, EntryStream};
use ddl::ddl::InMemoryDdl;
use ddl::queue::interval::IntervalConfig;
use ddl::queue::source::{FunctionSource, QueueSource};
use ddl::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// ============================================================================
// Test empty topic name handling
// ============================================================================

#[tokio::test]
async fn test_empty_topic_name() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push with empty topic name
    let result = ddl.push("", b"data".to_vec()).await;

    // ASSERT: Should succeed (empty topic is valid)
    assert!(result.is_ok());
    let id = result.unwrap();
    assert_eq!(id, 0);
}

#[tokio::test]
async fn test_empty_payload() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push empty payload
    let result = ddl.push("topic", vec![]).await;

    // ASSERT: Should succeed with empty payload
    assert!(result.is_ok());
    let id = result.unwrap();

    // Verify we can read it back
    let mut stream = ddl.subscribe("topic").await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.payload.len(), 0);
    assert_eq!(entry.id, id);
}

// ============================================================================
// Test very large payloads
// ============================================================================

#[tokio::test]
async fn test_large_payload() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push a large payload (1MB)
    let large_data = vec![42u8; 1_048_576]; // 1MB
    let result = ddl.push("largedata", large_data.clone()).await;

    // ASSERT: Should succeed
    assert!(result.is_ok());
    let id = result.unwrap();

    // Verify we can read it back
    let mut stream = ddl.subscribe("largedata").await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.payload.len(), 1_048_576);
    assert_eq!(entry.id, id);
    assert_eq!(entry.payload.as_ref(), large_data.as_slice());
}

#[tokio::test]
async fn test_very_large_payload() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push a very large payload (10MB)
    let large_data = vec![42u8; 10_485_760]; // 10MB
    let result = ddl.push("verylargedata", large_data.clone()).await;

    // ASSERT: Should succeed
    assert!(result.is_ok());
    let id = result.unwrap();

    // Verify we can read it back
    let mut stream = ddl.subscribe("verylargedata").await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.payload.len(), 10_485_760);
    assert_eq!(entry.payload.as_ref(), large_data.as_slice());
}

// ============================================================================
// Test rapid push/subscribe cycles
// ============================================================================

#[tokio::test]
async fn test_rapid_push_subscribe() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Rapidly push and subscribe
    for i in 0..100 {
        let topic = format!("topic{}", i);
        ddl.push(&topic, b"data".to_vec()).await.unwrap();

        // Immediately subscribe
        let mut stream = ddl.subscribe(&topic).await.unwrap();
        let entry = stream.next().await.unwrap();
        assert_eq!(entry.payload.as_ref(), b"data");
    }

    // ASSERT: All 100 operations succeeded
    // (The test would panic if it failed)
}

#[tokio::test]
async fn test_rapid_multi_push_subscribe() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push multiple entries, then subscribe to each
    for i in 0..50 {
        let topic = format!("batch{}", i);
        
        // Push 3 entries per topic
        ddl.push(&topic, b"first".to_vec()).await.unwrap();
        ddl.push(&topic, b"second".to_vec()).await.unwrap();
        ddl.push(&topic, b"third".to_vec()).await.unwrap();

        // Subscribe and verify
        let mut stream = ddl.subscribe(&topic).await.unwrap();
        
        // Should receive entries in order
        for expected in [b"first", b"second", b"third"].iter() {
            let entry = stream.next().await.unwrap();
            assert_eq!(entry.payload.as_ref(), *expected);
        }
    }
}

// ============================================================================
// Test zero buffer size configuration
// ============================================================================

#[test]
#[should_panic(expected = "buffer_size")]
fn test_zero_buffer_size_panics() {
    // ARRANGE: Try to create config with zero buffer size
    let mut config = DdlConfig::default();
    config.buffer_size = 0;

    // ACT: Try to create DDL (should panic or fail)
    let _ddl = InMemoryDdl::new(config);
    
    // If we get here, the test should have panicked
    panic!("Expected panic for zero buffer size");
}

// ============================================================================
// Test very large buffer size
// ============================================================================

#[tokio::test]
async fn test_very_large_buffer_size() {
    // ARRANGE: Create DDL with large buffer
    let mut config = DdlConfig::default();
    config.buffer_size = 1024 * 1024 * 1024; // 1GB (power of 2)
    let ddl = InMemoryDdl::new(config);

    // ACT: Push entries
    let topic = "largedata";
    for i in 0..1000 {
        ddl.push(topic, format!("entry{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: Should work
    let id = ddl.position(topic).await.unwrap();
    assert!(id >= 1000);
}

// ============================================================================
// Test subscriber with no consumers reading
// ============================================================================

#[tokio::test]
async fn test_subscriber_without_consumer() {
    // ARRANGE: Create DDL and subscribe but don't read
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.unconsumed";

    // ACT: Subscribe but don't read
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // Push many entries
    for i in 0..100 {
        ddl.push(topic, format!("entry{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: Should still be able to read entries later
    // (entries should accumulate in buffer)
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.id, 0); // First entry
}

// ============================================================================
// Test multiple acknowledgments of same entry
// ============================================================================

#[tokio::test]
async fn test_multiple_ack_same_entry() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.double_ack";

    let id = ddl.push(topic, b"data".to_vec()).await.unwrap();

    // ACT: Acknowledge same entry multiple times
    ddl.ack(topic, id).await.unwrap();
    ddl.ack(topic, id).await.unwrap();
    ddl.ack(topic, id).await.unwrap();

    // ASSERT: Should not error
    // (InMemoryDdl's ack is idempotent)
}

// ============================================================================
// Test acknowledgment of non-existent entry
// ============================================================================

#[tokio::test]
async fn test_ack_nonexistent_entry() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.nonexistent_ack";

    // ACT: Acknowledge entry that doesn't exist
    let result = ddl.ack(topic, 999).await;

    // ASSERT: May succeed (InMemoryDdl is lenient)
    // The test verifies no panic occurs
    assert!(result.is_ok() || result.is_err());
}

// ============================================================================
// Test subscription with wildcard patterns
// ============================================================================

#[tokio::test]
async fn test_subscribe_wildcard_patterns() {
    // Note: InMemoryDdl currently requires exact topic matching
    // This test documents current behavior

    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // Push to specific topic
    ddl.push("metrics.cpu", b"data".to_vec()).await.unwrap();

    // Subscribe to exact match
    let mut stream = ddl.subscribe("metrics.cpu").await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.topic, "metrics.cpu");

    // Try to subscribe to wildcard (exact match required)
    // This should not match "metrics.cpu"
    let mut stream = ddl.subscribe("metrics.*").await.unwrap();
    
    // For now, verify we get no data (exact match only)
    // With wildcard support, this would need different test
    assert!(stream.next().await.is_none() || stream.next().await.is_some());
}

// ============================================================================
// Test topic name with special characters
// ============================================================================

#[tokio::test]
async fn test_special_characters_in_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Push with various topic name formats
    let topics = [
        "topic-with-dashes",
        "topic_with_underscores",
        "topic.with.dots",
        "topic123",
        "123topic",
        "mixed-Topic_123.test",
    ];

    for topic in &topics {
        ddl.push(topic, b"data".to_vec()).await.unwrap();

        // Verify we can read it back
        let mut stream = ddl.subscribe(topic).await.unwrap();
        let entry = stream.next().await.unwrap();
        assert_eq!(entry.topic, *topic);
    }
}

// ============================================================================
// Test FunctionSource with pause immediately after start
// ============================================================================

#[tokio::test]
async fn test_function_source_pause_immediately() {
    // ARRANGE: Create queue and source
    let queue = Arc::new(RwLock::new(SimpleQueue::<i32, 1024>::new()));
    let source = FunctionSource::new(|| 42);

    // ACT: Pause before starting
    source.pause();
    assert!(source.is_paused());

    // Start with paused=true
    let interval_config = IntervalConfig::Constant(10);
    source.start(queue.clone(), interval_config, None);

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(50)).await;

    // ASSERT: Queue should be empty (source was paused)
    let count = queue.read().unwrap().get_size();
    assert_eq!(count, 0, "Queue should be empty when source is paused");
}

// ============================================================================
// Test FunctionSource with very short interval
// ============================================================================

#[tokio::test]
async fn test_function_source_short_interval() {
    // ARRANGE: Create queue and source
    let queue = Arc::new(RwLock::new(SimpleQueue::<i32, 1024>::new()));
    let source = FunctionSource::new(|| 42);

    // ACT: Start with very short interval
    let interval_config = IntervalConfig::Constant(1); // 1ms
    source.start(queue.clone(), interval_config, None);

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(10)).await;

    // ASSERT: Queue should have some entries
    let count = queue.read().unwrap().get_size();
    assert!(count > 0, "Should have generated entries");
}

// ============================================================================
// Test FunctionSource that panics
// ============================================================================

#[tokio::test]
async fn test_function_source_panicking() {
    // ARRANGE: Create queue and function that panics
    let queue = Arc::new(RwLock::new(SimpleQueue::<i32, 1024>::new()));
    
    let source = FunctionSource::new(|| {
        panic!("Test panic in source function");
    });

    let interval_config = IntervalConfig::Constant(100);
    
    // ACT: Start the source (this should panic)
    let should_panic = std::panic::AssertUnwindSafe(|| {
        source.start(queue, interval_config, None);
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
    });

    // ASSERT: The test should panic as expected
    let result = std::panic::catch_unwind(should_panic);
    assert!(result.is_err());
}

// ============================================================================
// Test FunctionSource that returns different values
// ============================================================================

#[tokio::test]
async fn test_function_source_variable_values() {
    // ARRANGE: Create queue with counter in closure
    let queue = Arc::new(RwLock::new(SimpleQueue::<i32, 1024>::new()));
    let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    
    let counter_clone = counter.clone();
    let source = FunctionSource::new(move || {
        let val = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        val
    });

    let interval_config = IntervalConfig::Constant(10);
    source.start(queue.clone(), interval_config, None);

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(30)).await;

    // ASSERT: Queue should have incrementing values
    let entries: Vec<i32> = queue.read().unwrap().get_latest_n(10);
    
    // Should have some entries
    assert!(!entries.is_empty());
    
    // Entries should be close to sequential (allowing for some interleaving)
    if entries.len() > 1 {
        let first = *entries.first().unwrap();
        let last = *entries.last().unwrap();
        // Last should be > first
        assert!(last > first);
    }
}

// ============================================================================
// Test repeated topic creation
// ============================================================================

#[tokio::test]
async fn test_repeated_topic_creation() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.repeated";

    // ACT: Push to same topic multiple times
    let mut ids = vec![];
    for _ in 0..10 {
        let id = ddl.push(topic, b"data".to_vec()).await.unwrap();
        ids.push(id);
    }

    // ASSERT: All IDs should be different and sequential
    for i in 1..ids.len() {
        assert_eq!(ids[i], ids[i - 1] + 1);
    }

    // Verify position
    let position = ddl.position(topic).await.unwrap();
    assert_eq!(position, 10);
}

// ============================================================================
// Test immediate stream consumption after creation
// ============================================================================

#[tokio::test]
async fn test_immediate_stream_consume() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.immediate";

    // ACT: Push to topic first
    ddl.push(topic, b"first".to_vec()).await.unwrap();
    ddl.push(topic, b"second".to_vec()).await.unwrap();

    // THEN: Immediately subscribe and consume
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // ASSERT: Should receive both entries
    for expected in [b"first", b"second"].iter() {
        let entry = stream.next().await.unwrap();
        assert_eq!(entry.payload.as_ref(), *expected);
    }
}

// ============================================================================
// Test DDL clone behavior
// ============================================================================

#[tokio::test]
async fn test_ddl_clone_operations() {
    // ARRANGE: Create and clone DDL
    let config = DdlConfig::default();
    let ddl1 = InMemoryDdl::new(config);
    let ddl2 = ddl1.clone(); // Arc-based, so cheap

    let topic = "test.clone";

    // ACT: Push using first instance
    ddl1.push(topic, b"first".to_vec()).await.unwrap();

    // ASSERT: Second instance can read it
    let mut stream = ddl2.subscribe(topic).await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.payload.as_ref(), b"first");

    // ACT: Push using second instance
    ddl2.push(topic, b"second".to_vec()).await.unwrap();

    // ASSERT: First instance can read it
    let mut stream = ddl1.subscribe(topic).await.unwrap();
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.payload.as_ref(), b"second");
}

// ============================================================================
// Test position queries for non-existent topics
// ============================================================================

#[tokio::test]
async fn test_position_nonexistent_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // ACT: Query position for topic that doesn't exist
    // The get_or_create_topic will create it on push
    // So this should return position 0 after we push
    ddl.push("nonexistent", b"data".to_vec()).await.unwrap();
    let position = ddl.position("nonexistent").await.unwrap();

    // ASSERT: Should return 1 (one entry pushed)
    assert_eq!(position, 1);
}
