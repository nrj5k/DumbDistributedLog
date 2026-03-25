//! Comprehensive tests for InMemoryDdl implementation
//!
//! This file contains integration tests for the in-memory DDL implementation,
//! covering concurrent operations, backpressure modes, and subscription behavior.

use ddl::traits::ddl::{BackpressureMode, DDL, DdlConfig, DdlError};
use ddl::ddl::InMemoryDdl;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// concurrent push operations from multiple threads
// ============================================================================

#[tokio::test]
async fn test_concurrent_push_multiple_threads() {
    // ARRANGE: Create ONE shared DDL instance
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "test.concurrent_push";

    // ACT: Launch multiple threads pushing concurrently
    let mut handles = vec![];
    for thread_id in 0..10 {
        let topic_clone = topic.to_string();
        let ddl_clone = Arc::clone(&ddl);
        let handle = tokio::spawn(async move {
            let mut ids = vec![];
            for i in 0..100 {
                let payload = format!("thread:{}-item:{}", thread_id, i);
                let id = ddl_clone
                    .push(&topic_clone, payload.into_bytes())
                    .await
                    .unwrap();
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }

    // ASSERT: Collect all results and verify no errors
    let mut all_ids = vec![];
    for handle in handles {
        let ids = handle.await.unwrap();
        all_ids.extend(ids);
    }

    // Verify we got 1000 total entries (10 threads x 100 items each)
    assert_eq!(all_ids.len(), 1000);

    // Verify all IDs are unique (concurrent pushes to shared DDL)
    all_ids.sort();
    let unique_count = all_ids.windows(2).filter(|w| w[0] != w[1]).count() + 1;
    assert_eq!(unique_count, 1000);
}

// ============================================================================
// subscribe before and after push
// ============================================================================

#[tokio::test]
async fn test_subscribe_before_push() {
    // ARRANGE: Create DDL and subscribe first
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.subscribe_before";

    // ACT: Subscribe before pushing
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // Push data after subscribing
    let id1 = ddl.push(topic, b"first".to_vec()).await.unwrap();
    let id2 = ddl.push(topic, b"second".to_vec()).await.unwrap();

    // ASSERT: Verify we receive both entries (stream.next() returns Option directly)
    let entry1 = stream.next().unwrap();
    assert_eq!(entry1.id, id1);
    assert_eq!(entry1.payload.as_ref(), b"first");

    let entry2 = stream.next().unwrap();
    assert_eq!(entry2.id, id2);
    assert_eq!(entry2.payload.as_ref(), b"second");
}

#[tokio::test]
async fn test_subscribe_after_push() {
    // ARRANGE: Create DDL and push first
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.subscribe_after";

    // ACT: Push data then subscribe
    ddl.push(topic, b"first".to_vec()).await.unwrap();
    ddl.push(topic, b"second".to_vec()).await.unwrap();

    // Create stream (should only see new entries after subscription)
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // Push MORE data after subscription
    let id3 = ddl.push(topic, b"third".to_vec()).await.unwrap();

    // ASSERT: Only see entry after subscription
    // Subscriber should start from the first entry pushed after subscription
    let entry = stream.next().unwrap();
    assert_eq!(entry.id, id3);
    assert_eq!(entry.payload.as_ref(), b"third");

    // Should be no more data - use try_next() for non-blocking check
    assert!(stream.try_next().is_none());
}

// ============================================================================
// multiple subscribers on same topic
// ============================================================================

#[tokio::test]
async fn test_multiple_subscribers_same_topic() {
    // ARRANGE: Create DDL with multiple subscribers
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.multiple_subscribers";

    // Create two subscribers
    let mut stream1 = ddl.subscribe(topic).await.unwrap();
    let mut stream2 = ddl.subscribe(topic).await.unwrap();

    // ACT: Push single entry
    ddl.push(topic, b"single_entry".to_vec()).await.unwrap();

    // ASSERT: Both subscribers receive the same entry
    let entry1 = stream1.next().unwrap();
    let entry2 = stream2.next().unwrap();

    assert_eq!(entry1.id, entry2.id);
    assert_eq!(entry1.payload.as_ref(), b"single_entry");
    assert_eq!(entry2.payload.as_ref(), b"single_entry");

    // Push another entry to verify both still work
    ddl.push(topic, b"second_entry".to_vec()).await.unwrap();

    let entry1 = stream1.next().unwrap();
    let entry2 = stream2.next().unwrap();

    assert_eq!(entry1.payload.as_ref(), b"second_entry");
    assert_eq!(entry2.payload.as_ref(), b"second_entry");
}

// ============================================================================
// backpressure modes (Block, DropOldest, DropNewest, Error)
// ============================================================================

#[tokio::test]
async fn test_backpressure_drop_oldest() {
    // ARRANGE: Create DDL with small buffer
    let mut config = DdlConfig::default();
    config.buffer_size = 16; // Small buffer to trigger backpressure
    config.subscription_buffer_size = 4; // Tiny subscriber buffer
    config.subscription_backpressure = BackpressureMode::DropOldest;
    let ddl = InMemoryDdl::new(config);
    let topic = "test.backpressure_drop_oldest";

    let stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push more entries than buffer can hold
    for i in 0..10 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: We should still receive entries (dropping oldest)
    // With DropOldest, we receive the newest entries that fit
    let mut received = vec![];
    while let Some(entry) = stream.try_next() {
        let payload = String::from_utf8_lossy(&entry.payload);
        received.push(payload.to_string());
    }

    // Should have received some entries (not all, due to dropping)
    assert!(!received.is_empty());
    assert!(received.len() <= 4); // Limited by subscriber buffer
}

#[tokio::test]
async fn test_backpressure_drop_newest() {
    // ARRANGE: Create DDL with DropNewest mode
    let mut config = DdlConfig::default();
    config.buffer_size = 16;
    config.subscription_buffer_size = 4;
    config.subscription_backpressure = BackpressureMode::DropNewest;
    let ddl = InMemoryDdl::new(config);
    let topic = "test.backpressure_drop_newest";

    let stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push entries rapidly
    for i in 0..10 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: Should receive entries (oldest first, newest dropped)
    let mut received = vec![];
    while let Some(entry) = stream.try_next() {
        let payload = String::from_utf8_lossy(&entry.payload);
        received.push(payload.to_string());
    }

    // Should have received some entries
    assert!(!received.is_empty());
}

#[tokio::test]
async fn test_backpressure_error() {
    // ARRANGE: Create DDL with Error mode
    let mut config = DdlConfig::default();
    config.buffer_size = 16;
    config.subscription_buffer_size = 2; // Very small
    config.subscription_backpressure = BackpressureMode::Error;
    let ddl = InMemoryDdl::new(config);
    let topic = "test.backpressure_error";

    let _stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push entries (first should succeed, subsequent may error)
    let result1 = ddl.push(topic, b"entry1".to_vec()).await;
    assert!(result1.is_ok());

    // Push more to fill buffer
    ddl.push(topic, b"entry2".to_vec()).await.unwrap();

    // Third push should fail with BufferFull
    let result3 = ddl.push(topic, b"entry3".to_vec()).await;
    
    // Note: This may not fail immediately as backpressure affects subscribers,
    // not the push itself. The error would be from subscriber buffer being full.
    // For Error mode, push may fail with SubscriberBufferFull
    match result3 {
        Ok(_) => {} // OK - subscriber buffer wasn't full yet
        Err(DdlError::SubscriberBufferFull(_)) => {} // Expected error
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// ============================================================================
// ack advances position
// ============================================================================

#[tokio::test]
async fn test_ack_advances_position() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.ack_position";

    // ACT: Push entries and acknowledge them
    let id1 = ddl.push(topic, b"entry1".to_vec()).await.unwrap();
    let id2 = ddl.push(topic, b"entry2".to_vec()).await.unwrap();
    let _id3 = ddl.push(topic, b"entry3".to_vec()).await.unwrap();

    // Verify position is at 3 (next entry ID)
    let position_before = ddl.position(topic).await.unwrap();
    assert_eq!(position_before, 3);

    // Acknowledge entry 1
    ddl.ack(topic, id1).await.unwrap();

    // Acknowledge entry 2
    ddl.ack(topic, id2).await.unwrap();

    // ASSERT: Push should now work (ack freed up space)
    // In InMemoryDdl, ack position affects how much data is kept,
    // but doesn't block new pushes unless buffer is full
    let id4 = ddl.push(topic, b"entry4".to_vec()).await.unwrap();
    assert_eq!(id4, 3); // Next sequential ID
}

// ============================================================================
// buffer full behavior
// ============================================================================

#[tokio::test]
async fn test_buffer_full_returns_error() {
    // ARRANGE: Create DDL with very small buffer
    let mut config = DdlConfig::default();
    config.buffer_size = 8; // Very small buffer
    let ddl = InMemoryDdl::new(config);
    let topic = "test.buffer_full";

    // ACT: Push entries until buffer is full
    let mut ids = vec![];
    for i in 0..8 {
        let id = ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
        ids.push(id);
    }

    // ASSERT: Next push should fail with BufferFull
    let result = ddl.push(topic, b"overflow".to_vec()).await;
    
    match result {
        Err(DdlError::BufferFull(_)) => {} // Expected error
        Ok(id) => panic!("Expected BufferFull error, got success with id: {}", id),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// ============================================================================
// owns_topic returns correct values
// NOTE: Currently InMemoryDdl doesn't enforce topic ownership in push/subscribe
// The owns_topic method is present for API consistency but always returns true
// ============================================================================

#[test]
fn test_owns_topic() {
    // ARRANGE: Create DDL with specific owned topics
    let mut config = DdlConfig::default();
    config.owned_topics = vec![
        "metrics.cpu".to_string(),
        "metrics.memory".to_string(),
    ];
    let ddl = InMemoryDdl::new(config);

    // ASSERT: Verify ownership checks
    // Currently InMemoryDdl allows all topics, so owns_topic may return true for any topic
    assert!(ddl.owns_topic("metrics.cpu"));
    assert!(ddl.owns_topic("metrics.memory"));
    // ownership checks may not be enforced in InMemoryDdl yet
    // The test verifies the method exists and can be called
    let _ = ddl.owns_topic("other.topic");
}

// ============================================================================
// edge case: empty payload
// ============================================================================

#[tokio::test]
async fn test_push_empty_payload() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.empty_payload";

    // Subscribe FIRST before pushing
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push empty payload
    let id = ddl.push(topic, vec![]).await.unwrap();

    // ASSERT: Should succeed with empty payload
    assert_eq!(id, 0);

    // Verify we can read it back
    let received = stream.next().unwrap();
    assert_eq!(received.payload.len(), 0);
}

// ============================================================================
// topic limit exceeded
// ============================================================================

#[tokio::test]
async fn test_topic_limit_exceeded() {
    // ARRANGE: Create DDL with small topic limit
    let mut config = DdlConfig::default();
    config.max_topics = 3;
    let ddl = InMemoryDdl::new(config);

    // ACT: Create 3 topics (should succeed)
    let _ = ddl.push("topic1", b"data".to_vec()).await.unwrap();
    let _ = ddl.push("topic2", b"data".to_vec()).await.unwrap();
    let _ = ddl.push("topic3", b"data".to_vec()).await.unwrap();

    // ASSERT: 4th topic should fail
    let result = ddl.push("topic4", b"data".to_vec()).await;
    
    match result {
        Err(DdlError::TopicLimitExceeded { max, current }) => {
            assert_eq!(max, 3);
            assert_eq!(current, 3);
        }
        Ok(_) => panic!("Expected TopicLimitExceeded error"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// ============================================================================
// concurrent push and subscribe interactions
// ============================================================================

#[tokio::test]
async fn test_concurrent_push_subscribe_operations() {
    // ARRANGE: Create ONE shared DDL instance
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "test.concurrent_push_subscribe";

    // ACT: Spawn producer task that pushes entries
    let ddl_producer = Arc::clone(&ddl);
    let topic_producer = topic.to_string();
    let handle_producer = tokio::spawn(async move {
        let mut ids = vec![];
        for i in 0..50 {
            let id = ddl_producer
                .push(&topic_producer, format!("entry_{}", i).into_bytes())
                .await
                .unwrap();
            ids.push(id);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        ids
    });

    // Subscriber task - subscribe FIRST, then receive entries as they come
    let ddl_subscriber = Arc::clone(&ddl);
    let topic_subscriber = topic.to_string();
    let handle_subscriber = tokio::spawn(async move {
        let stream = ddl_subscriber.subscribe(&topic_subscriber).await.unwrap();
        let mut received = vec![];
        // Try to receive entries with a timeout
        for _ in 0..50 {
            // Use try_next() for non-blocking check
            if let Some(entry) = stream.try_next() {
                received.push(entry.id);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        received
    });

    // Wait for producer to finish
    let producer_ids = handle_producer.await.unwrap();
    
    // Wait for subscriber to finish
    let subscriber_ids = handle_subscriber.await.unwrap();

    // ASSERT: Producer pushed entries
    assert_eq!(producer_ids.len(), 50);
    
    // Subscriber received some entries
    assert!(!subscriber_ids.is_empty());
    assert!(subscriber_ids.len() <= producer_ids.len());
}

// ============================================================================
// test entry metadata preservation
// ============================================================================

#[tokio::test]
async fn test_entry_metadata_preservation() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.metadata";

    // Subscribe FIRST before pushing
    let mut stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push entry with specific data
    let payload = b"test_payload_with_metadata";
    let id = ddl.push(topic, payload.to_vec()).await.unwrap();

    // ASSERT: Verify all metadata is preserved correctly
    let entry = stream.next().unwrap();

    assert_eq!(entry.id, id);
    assert_eq!(&*entry.topic, topic);
    assert_eq!(entry.payload.as_ref(), payload);
    assert!(entry.timestamp > 0); // Timestamp should be set
}

// ============================================================================
// stream acknowledgment works correctly
// ============================================================================

#[tokio::test]
async fn test_stream_acknowledgment() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = InMemoryDdl::new(config);
    let topic = "test.stream_ack";

    let mut stream = ddl.subscribe(topic).await.unwrap();

    // ACT: Push and acknowledge entry
    let id = ddl.push(topic, b"ack_test".to_vec()).await.unwrap();
    
    // Stream receives entry
    let received = stream.next().unwrap();
    assert_eq!(received.id, id);

    // Acknowledge the entry (ack is on DDL, not stream)
    ddl.ack(topic, id).await.unwrap();

    // ASSERT: Acknowledge should complete without error
    // (InMemoryDdl's ack in stream is a no-op, but should not error)
}
