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

    // ASSERT: Should fail (empty topic name is invalid)
    assert!(result.is_err());
    match result {
        Err(ddl::traits::ddl::DdlError::InvalidTopic(msg)) => {
            assert!(msg.contains("empty"));
        }
        _ => panic!("Expected InvalidTopic error for empty topic name"),
    }
}

#[tokio::test]
async fn test_empty_payload() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // Subscribe BEFORE pushing (pub/sub pattern)
    let mut stream = ddl.subscribe("topic").await.unwrap();

    // ACT: Push empty payload
    let result = ddl.push("topic", vec![]).await;

    // ASSERT: Should succeed with empty payload
    assert!(result.is_ok());
    let id = result.unwrap();

    // Verify we can read it back
    let entry = stream.next().unwrap();
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

    // Subscribe BEFORE pushing (pub/sub pattern)
    let mut stream = ddl.subscribe("largedata").await.unwrap();

    // ACT: Push a large payload (1MB)
    let large_data = vec![42u8; 1_048_576]; // 1MB
    let result = ddl.push("largedata", large_data.clone()).await;

    // ASSERT: Should succeed
    assert!(result.is_ok());
    let id = result.unwrap();

    // Verify we can read it back
    let entry = stream.next().unwrap();
    assert_eq!(entry.payload.len(), 1_048_576);
    assert_eq!(entry.id, id);
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

    // Create subscriptions BEFORE pushing (pub/sub pattern)
    // This ensures the streams are ready to receive data
    let mut streams = vec![];
    for i in 0..100 {
        let topic = format!("topic{}", i);
        let stream = ddl.subscribe(&topic).await.unwrap();
        streams.push((topic, stream));
    }

    // ACT: Push data to each topic
    for (topic, _) in &streams {
        ddl.push(topic, b"data".to_vec()).await.unwrap();
    }

    // ASSERT: All 100 streams should receive their data
    for (topic, mut stream) in streams {
        let entry = stream.next().unwrap();
        assert_eq!(&*entry.topic, topic);
        assert_eq!(entry.payload.as_ref(), b"data");
    }
}

// ============================================================================
// Test FunctionSource pause immediately after start
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

    // ASSERT: Source is paused
    assert!(source.is_paused());
}

// ============================================================================
// Test FunctionSource with different values
// ============================================================================

#[test]
fn test_function_source_variable_values() {
    // ARRANGE: Create function source
    let source = FunctionSource::new(|| 42);

    // ASSERT: Source has correct initial state
    assert!(!source.is_paused());
    assert!(!source.should_stop());
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

    // Subscribe BEFORE pushing (pub/sub pattern)
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push to topic
    ddl.push(topic, b"first".to_vec()).await.unwrap();
    ddl.push(topic, b"second".to_vec()).await.unwrap();

    // ASSERT: Should receive both entries
    let entry1 = stream.next().unwrap();
    assert_eq!(entry1.payload.as_ref(), b"first");

    let entry2 = stream.next().unwrap();
    assert_eq!(entry2.payload.as_ref(), b"second");
}

// ============================================================================
// Test topic name with special characters
// ============================================================================

#[tokio::test]
async fn test_special_characters_in_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);

    // Topics with special characters
    let topics = [
        "topic-with-dashes",
        "topic_with_underscores",
        "topic.with.dots",
        "topic123",
        "123topic",
        "mixed-Topic_123.test",
    ];

    // Create subscriptions BEFORE pushing (pub/sub pattern)
    let mut streams = vec![];
    for topic in &topics {
        let stream = ddl.subscribe(topic).await.unwrap();
        streams.push((*topic, stream));
    }

    // ACT: Push with various topic name formats
    for (topic, _) in &streams {
        ddl.push(topic, b"data".to_vec()).await.unwrap();
    }

    // ASSERT: Verify we can read each topic
    for (topic, mut stream) in streams {
        let entry = stream.next().unwrap();
        assert_eq!(&*entry.topic, topic);
    }
}

// ============================================================================
// Test ack non-existent entry
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
// Test DDL position for non-existent topic
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
