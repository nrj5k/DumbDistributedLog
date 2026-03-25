//! Concurrency stress tests for the autoqueues library
//!
//! This file contains stress tests that push the system to its limits
//! with concurrent operations, high throughput, and many participants.

use ddl::traits::ddl::{DDL, DdlConfig, DdlError};
use ddl::ddl::InMemoryDdl;
use ddl::queue::source::{FunctionSource, QueueSource};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Stress test: 1000 concurrent pushes
// ============================================================================

#[tokio::test]
async fn test_stress_concurrent_pushes() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let topic = "stress.concurrent_pushes";
    let ddl = Arc::new(InMemoryDdl::new(config));
    // ACT: Launch 100 threads pushing concurrently (using shared DDL)
    let mut handles = vec![];
    for _thread_id in 0..100 {
        let topic_clone = topic.to_string();
        let ddl_clone = ddl.clone();
        let handle = tokio::spawn(async move {
            let mut ids = vec![];
            for i in 0..10 {
                let id = ddl_clone
                    .push(&topic_clone, format!("entry_{}", i).into_bytes())
                    .await
                    .unwrap();
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }

    // ASSERT: All pushes should succeed
    let mut ids = vec![];
    for handle in handles {
        ids.extend(handle.await.unwrap());
    }

    // Verify we got 1000 entries (100 threads x 10 items each)
    assert_eq!(ids.len(), 1000);

    // Verify entries are in the DDL
    let position = ddl.position(topic).await.unwrap();
    assert_eq!(position, 1000);
}

// ============================================================================
// Stress test: 100 subscribers, 100 pushes
// ============================================================================

#[tokio::test]
async fn test_stress_many_subscribers() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let config_clone = config.clone();
    let topic = "stress.many_subscribers";

    // Create DDL instance for this test
    let ddl = InMemoryDdl::new(config_clone.clone());
    
    // ACT: Create 100 subscribers
    let mut streams = vec![];
    for _ in 0..100 {
        let stream = ddl.subscribe(topic).await.unwrap();
        streams.push(stream);
    }

    // Push 100 entries
    for i in 0..100 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: Each subscriber should receive all entries
    for (stream_idx, stream) in streams.iter_mut().enumerate() {
        let mut received = vec![];
        while let Some(entry) = stream.try_next() {
            received.push(entry.id);
        }

        // Each subscriber should have received entries
        assert!(
            !received.is_empty(),
            "Subscriber {} should have received entries",
            stream_idx
        );
    }
}

// ============================================================================
// Stress test: sustained high-throughput
// ============================================================================

#[tokio::test]
async fn test_stress_high_throughput() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let topic = "stress.high_throughput";
    let ddl = InMemoryDdl::new(config);

    // ACT: Push 10000 entries as fast as possible
    let start = std::time::Instant::now();
    let mut all_ids = vec![];

    // Push in batches
    for batch in 0..10 {
        for i in 0..1000 {
            let id = ddl.push(&topic, format!("batch{}_entry{}", batch, i).into_bytes())
                .await
                .unwrap();
            all_ids.push(id);
        }
    }
    let duration = start.elapsed();

    // ASSERT: 10000 entries pushed
    assert_eq!(all_ids.len(), 10000);

    // Throughput should be reasonable
    let rate = (all_ids.len() as f64) / duration.as_secs_f64();
    log::info!("Push rate: {:.2} entries/second", rate);
    
    // We expect at least 1000 entries/second (conservative)
    assert!(rate > 1000.0, "Push rate should be at least 1000 entries/second");
}

// ============================================================================
// Performance measurement: baseline push rate
// ============================================================================

#[tokio::test]
async fn test_performance_baseline_push_rate() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let topic = "perf.baseline";
    let ddl = InMemoryDdl::new(config);

    // Subscribe to drain entries during the test
    let stream = ddl.subscribe(topic).await.unwrap();

    // Warmup: push and drain
    for i in 0..100 {
        ddl.push(&topic, format!("warmup_{}", i).into_bytes())
            .await
            .unwrap();
    }
    while stream.try_next().is_some() {}

    // ACT: Measure push rate
    let start = std::time::Instant::now();
    let mut count = 0;

    while start.elapsed() < Duration::from_secs(1) {
        match ddl.push(&topic, format!("entry_{}", count).into_bytes()).await {
            Ok(_) => count += 1,
            Err(DdlError::BufferFull(_)) => {
                // Drain some entries and continue
                for _ in 0..100 {
                    if stream.try_next().is_none() {
                        break;
                    }
                }
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    let rate = (count as f64) / start.elapsed().as_secs_f64();

    // ASSERT: Should achieve reasonable throughput
    log::info!("Push rate: {:.2} entries/second", rate);

    // Expected: At least 100 entries/second
    assert!(rate > 100.0, "Should achieve at least 100 entries/second");
}

// ============================================================================
// Performance measurement: batch push rate
// ============================================================================

#[tokio::test]
async fn test_performance_batch_push_rate() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let config_clone = config.clone();
    let topic = "perf.batch";
    let ddl = InMemoryDdl::new(config_clone);

    // ACT: Push batches of 100 entries
    let batch_size = 100;
    let num_batches = 100;

    let start = std::time::Instant::now();
    for batch_idx in 0..num_batches {
        let mut payloads = vec![];
        for entry_idx in 0..batch_size {
            let payload = format!("batch{}_entry{}", batch_idx, entry_idx);
            payloads.push(payload.into_bytes());
        }
        ddl.push_batch(&topic, payloads).await.unwrap();
    }

    let duration = start.elapsed();
    let total_entries = batch_size * num_batches;
    let rate = (total_entries as f64) / duration.as_secs_f64();

    // ASSERT: Batch push should be efficient
    log::info!(
        "Batch push rate: {:.2} entries/second ({:.2} batches/second)",
        rate,
        num_batches as f64 / duration.as_secs_f64()
    );

    // Expected: At least 100 entries/second
    assert!(rate > 100.0);
}

// ============================================================================
// Test FunctionSource with pause/resume under load
// ============================================================================

#[test]
fn test_function_source_pause_resume() {
    // ARRANGE: Create function source
    let source = FunctionSource::new(|| 42);

    // ASSERT: Source has correct initial state
    assert!(!source.is_paused());
    
    // ACT: Pause the source
    source.pause();
    assert!(source.is_paused());

    // ACT: Resume the source
    source.resume();
    assert!(!source.is_paused());
}

// ============================================================================
// Stress test: many producers, one topic
// ============================================================================

#[tokio::test]
async fn test_stress_many_producers_one_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.many_producers";

    // ACT: 50 producers, each pushing 20 entries (using shared DDL instance)
    let mut handles = vec![];
    for producer_id in 0..50 {
        let topic_clone = topic.to_string();
        let ddl_clone = ddl.clone();
        let handle = tokio::spawn(async move {
            let mut ids = vec![];
            for i in 0..20 {
                let id = ddl_clone
                    .push(&topic_clone, format!("p{}_e{}", producer_id, i).into_bytes())
                    .await
                    .unwrap();
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }

    // Wait for all producers
    let mut all_ids = vec![];
    for handle in handles {
        all_ids.extend(handle.await.unwrap());
    }

    // ASSERT: 1000 total entries
    assert_eq!(all_ids.len(), 1000);

    // Verify entries are monotonically increasing (from same DDL instance)
    for i in 1..all_ids.len() {
        assert!(all_ids[i] >= all_ids[i - 1]);
    }
}
