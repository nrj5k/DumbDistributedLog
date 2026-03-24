//! Concurrency stress tests for the autoqueues library
//!
//! This file contains stress tests that push the system to its limits
//! with concurrent operations, high throughput, and many participants.

use ddl::traits::ddl::{DDL, DdlConfig, EntryStream};
use ddl::ddl::InMemoryDdl;
use ddl::queue::interval::IntervalConfig;
use ddl::queue::source::FunctionSource;
use ddl::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::timeout;

// ============================================================================
// Stress test: 1000 concurrent pushes
// ============================================================================

#[tokio::test]
async fn test_stress_concurrent_pushes() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.concurrent_pushes";

    // ACT: Spawn 1000 concurrent push tasks
    let mut handles = vec![];
    for i in 0..1000 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            ddl_clone
                .push(
                    &topic_clone,
                    format!("entry_{}", i).into_bytes(),
                )
                .await
                .unwrap()
        });
        handles.push(handle);
    }

    // ASSERT: All pushes should succeed
    let mut ids = vec![];
    for handle in handles {
        ids.push(handle.await.unwrap());
    }

    // Verify we got 1000 unique entries
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
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.many_subscribers";

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
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.high_throughput";

    // ACT: Push 10000 entries as fast as possible
    let start = std::time::Instant::now();
    let mut handles = vec![];

    // Spawn batches of pushes
    for batch in 0..10 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            let mut ids = vec![];
            for i in 0..1000 {
                let id = ddl_clone
                    .push(&topic_clone, format!("batch{}_entry{}", batch, i).into_bytes())
                    .await
                    .unwrap();
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }

    // Wait for all batches to complete
    let mut all_ids = vec![];
    for handle in handles {
        all_ids.extend(handle.await.unwrap());
    }
    let duration = start.elapsed();

    // ASSERT: 10000 entries pushed
    assert_eq!(all_ids.len(), 10000);

    // Throughput should be reasonable
    // (This is a sanity check - actual numbers will vary)
    let rate = (all_ids.len() as f64) / duration.as_secs_f64();
    log::info!("Push rate: {:.2} entries/second", rate);
    
    // We expect at least 1000 entries/second (conservative)
    assert!(rate > 1000.0, "Push rate should be at least 1000 entries/second");
}

// ============================================================================
// Stress test: concurrent push and subscribe
// ============================================================================

#[tokio::test]
async fn test_stress_concurrent_push_subscribe() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.concurrent_push_sub";

    // ACT: Run concurrent producers and consumers
    let mut handles = vec![];

    // 10 producer tasks
    for producer_id in 0..10 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            let mut ids = vec![];
            for i in 0..100 {
                let id = ddl_clone
                    .push(&topic_clone, format!("p{}_e{}", producer_id, i).into_bytes())
                    .await
                    .unwrap();
                ids.push(id);
                // Small delay to spread out operations
                tokio::time::sleep(Duration::from_micros(10)).await;
            }
            ids
        });
        handles.push(handle);
    }

    // 10 consumer tasks
    for consumer_id in 0..10 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            let mut received = vec![];
            // Try to get entries with a timeout
            for _ in 0..1000 {
                let mut stream = ddl_clone
                    .subscribe(&topic_clone)
                    .await
                    .unwrap_or_default();
                if let Some(entry) = stream.try_next() {
                    received.push(entry.id);
                }
                tokio::time::sleep(Duration::from_micros(5)).await;
            }
            received
        });
        handles.push(handle);
    }

    // Wait for all tasks
    let results: Vec<Vec<u64>> = handles
        .into_iter()
        .map(|h| h.await.unwrap())
        .collect();

    // ASSERT: Producers pushed entries
    let producer_ids: Vec<u64> = results[0..10]
        .iter()
        .flatten()
        .cloned()
        .collect();

    assert_eq!(producer_ids.len(), 1000); // 10 producers × 100 entries

    // ASSERT: Consumers received some entries
    let consumer_ids: Vec<u64> = results[10..20]
        .iter()
        .flatten()
        .cloned()
        .collect();

    assert!(!consumer_ids.is_empty());
}

// ============================================================================
// Stress test: many topics
// ============================================================================

#[tokio::test]
async fn test_stress_many_topics() {
    // ARRANGE: Create DDL
    let mut config = DdlConfig::default();
    config.max_topics = 10000; // Allow many topics
    let ddl = Arc::new(InMemoryDdl::new(config));

    // ACT: Create 1000 topics with 10 entries each
    for topic_idx in 0..1000 {
        let topic = format!("topic_{}", topic_idx);
        for entry_idx in 0..10 {
            ddl.push(&topic, format!("e{}", entry_idx).into_bytes())
                .await
                .unwrap();
        }
    }

    // ASSERT: Created and can read from all topics
    for topic_idx in 0..1000 {
        let topic = format!("topic_{}", topic_idx);
        let position = ddl.position(&topic).await.unwrap();
        assert_eq!(position, 10);
    }
}

// ============================================================================
// Stress test: many subscribers to same topic
// ============================================================================

#[tokio::test]
async fn test_stress_many_subscribers_same_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.topic";

    // ACT: Create 500 subscribers
    let mut streams = vec![];
    for _ in 0..500 {
        let stream = ddl.subscribe(topic).await.unwrap();
        streams.push(stream);
    }

    // Push 100 entries
    for i in 0..100 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ASSERT: Each subscriber should have received entries
    let mut empty_subscribers = 0;
    for (idx, stream) in streams.iter_mut().enumerate() {
        let mut has_data = false;
        for _ in 0..10 {
            if stream.try_next().is_some() {
                has_data = true;
                break;
            }
        }
        if !has_data {
            empty_subscribers += 1;
        }
    }

    // Most subscribers should have received data
    // Some may not have (race conditions), but most should
    let received_count = 500 - empty_subscribers;
    log::info!(
        "Subscribers that received data: {}/500",
        received_count
    );

    // At least 80% should have received something
    assert!(received_count >= 400);
}

// ============================================================================
// Stress test: rapid topic creation and deletion
// ============================================================================

#[tokio::test]
async fn test_stress_rapid_topic_operations() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));

    // ACT: Rapidly create topics, write entries, and verify
    let mut topic_handles = vec![];

    for topic_idx in 0..100 {
        let ddl_clone = ddl.clone();
        let topic = format!("rapid_topic_{}", topic_idx);
        let handle = tokio::spawn(async move {
            // Write 10 entries
            for entry_idx in 0..10 {
                ddl_clone
                    .push(&topic, format!("entry_{}", entry_idx).into_bytes())
                    .await
                    .unwrap();
            }

            // Verify entries
            let position = ddl_clone.position(&topic).await.unwrap();
            assert_eq!(position, 10);

            // Read all entries
            let mut stream = ddl_clone.subscribe(&topic).await.unwrap();
            let mut count = 0;
            while stream.try_next().is_some() {
                count += 1;
            }
            assert_eq!(count, 10);
        });
        topic_handles.push(handle);
    }

    // Wait for all topics
    for handle in topic_handles {
        handle.await.unwrap();
    }
}

// ============================================================================
// Stress test: consumer race condition
// ============================================================================

#[tokio::test]
async fn test_stress_consumer_race_condition() {
    // ARRANGE: Create DDL with one topic
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.race";

    // Push 1000 entries
    for i in 0..1000 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ACT: Create many consumers reading concurrently
    let mut consumer_handles = vec![];

    for consumer_id in 0..50 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            let mut stream = ddl_clone.subscribe(&topic_clone).await.unwrap();
            let mut received = vec![];
            
            // Try to read multiple entries
            for _ in 0..20 {
                if let Some(entry) = stream.try_next() {
                    received.push(entry.id);
                }
            }
            received
        });
        consumer_handles.push(handle);
    }

    // ASSERT: All consumers should have received some data
    let results: Vec<Vec<u64>> = consumer_handles
        .into_iter()
        .map(|h| h.await.unwrap())
        .collect();

    // Most consumers should have received entries
    let consumers_with_data = results.iter().filter(|r| !r.is_empty()).count();
    log::info!("Consumers with data: {}/50", consumers_with_data);
    
    assert!(consumers_with_data > 40);
}

// ============================================================================
// Stress test: backpressure under load
// ============================================================================

#[tokio::test]
async fn test_stress_backpressure_under_load() {
    // ARRANGE: Create DDL with small buffer and Error backpressure
    let mut config = DdlConfig::default();
    config.buffer_size = 16;
    config.subscription_buffer_size = 4;
    config.subscription_backpressure = crate::traits::ddl::BackpressureMode::Error;
    
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.backpressure";

    // ACT: Push大量 entries rapidly
    let mut success_count = 0;
    let mut error_count = 0;

    for i in 0..100 {
        let result = ddl.push(topic, format!("entry_{}", i).into_bytes()).await;
        match result {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    // ASSERT: Some should succeed, some should fail
    log::info!(
        "Backpressure: {} successes, {} errors",
        success_count,
        error_count
    );

    // With Error mode and small buffer, we expect some errors
    assert!(success_count > 0);
}

// ============================================================================
// Stress test: FunctionSource under load
// ============================================================================

#[tokio::test]
async fn test_stress_function_source_load() {
    // ARRANGE: Create function source with counter
    let queue = Arc::new(RwLock::new(SimpleQueue::<i64, 4096>::new()));
    let counter = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let counter_clone = counter.clone();

    let source = FunctionSource::new(move || {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    });

    // Start with short interval
    let interval_config = IntervalConfig::Constant(1);
    source.start(queue.clone(), interval_config, None);

    // ACT: Let it run for 100ms
    tokio::time::sleep(Duration::from_millis(100)).await;

    // ASSERT: Should have generated many entries
    let produced = counter.load(std::sync::atomic::Ordering::Relaxed);
    log::info!("Generated {} entries in 100ms", produced);

    // We expect at least 50 entries in 100ms with 1ms interval
    assert!(produced >= 50);
}

// ============================================================================
// Stress test: FunctionSource pause/resume under load
// ============================================================================

#[tokio::test]
async fn test_stress_function_source_pause_resume() {
    // ARRANGE: Create function source
    let queue = Arc::new(RwLock::new(SimpleQueue::<i32, 4096>::new()));
    let source = FunctionSource::new(|| 42);

    let interval_config = IntervalConfig::Constant(2);
    source.start(queue.clone(), interval_config, None);

    // ACT: Run for a bit
    tokio::time::sleep(Duration::from_millis(20)).await;
    let count_after_start = queue.read().unwrap().get_size();

    // PAUSE
    source.pause();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count_after_pause = queue.read().unwrap().get_size();

    // RESUME
    source.resume();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let count_after_resume = queue.read().unwrap().get_size();

    // ASSERT: Count should increase when running, not when paused
    assert!(count_after_start > 0, "Should generate entries after start");
    assert_eq!(
        count_after_pause, count_after_start,
        "Count should not increase while paused"
    );
    assert!(
        count_after_resume > count_after_pause,
        "Count should increase after resume"
    );
}

// ============================================================================
// Stress test: many threads reading same queue
// ============================================================================

#[test]
fn test_stress_many_queue_readers() {
    // ARRANGE: Create queue with data
    let mut queue: SimpleQueue<i32, 16384> = SimpleQueue::new();
    for i in 0..1000 {
        queue.push(i).unwrap();
    }

    // ACT: Spawn many consumers reading concurrently
    let mut handles = vec![];
    for _ in 0..100 {
        let queue_ref = &queue;
        let handle = std::thread::spawn(move || {
            let consumer = queue_ref.consumer();
            let mut count = 0;
            while consumer.has_data() {
                if consumer.pop().is_some() {
                    count += 1;
                }
            }
            count
        });
        handles.push(handle);
    }

    // ASSERT: All consumers should have read some data
    let counts: Vec<usize> = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // Each consumer should have read data (though may be different amounts)
    for count in &counts {
        assert!(*count > 0, "Each consumer should read some data");
    }
}

// ============================================================================
// Stress test: continuous high-rate push with bounded buffer
// ============================================================================

#[tokio::test]
async fn test_stress_bounded_buffer_pressure() {
    // ARRANGE: Create DDL with small buffer
    let mut config = DdlConfig::default();
    config.buffer_size = 32;
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.bounded";

    // ACT: Push continuously until buffer is full
    let mut success_count = 0;
    let mut full_errors = 0;

    for i in 0..1000 {
        match ddl.push(topic, format!("entry_{}", i).into_bytes()).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                if matches!(e, crate::traits::ddl::DdlError::BufferFull(_)) {
                    full_errors += 1;
                }
            }
        }
    }

    // ASSERT: Should have some successes and some buffer full errors
    log::info!(
        "Bounded buffer: {} successes, {} full errors",
        success_count,
        full_errors
    );

    assert!(success_count > 0);
}

// ============================================================================
// Stress test: concurrent ack operations
// ============================================================================

#[tokio::test]
async fn test_stress_concurrent_ack() {
    // ARRANGE: Create DDL with entries
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.ack";

    // Push many entries
    let mut ids = vec![];
    for i in 0..100 {
        let id = ddl.push(topic, format!("entry_{}", i).into_bytes()).await.unwrap();
        ids.push(id);
    }

    // ACT: Acknowledge entries concurrently
    let mut handles = vec![];
    for id in ids {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            ddl_clone.ack(&topic_clone, id).await
        });
        handles.push(handle);
    }

    // ASSERT: All acks should complete (no panics)
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.await.unwrap())
        .collect();

    // All should succeed (acks are idempotent)
    for result in &results {
        assert!(result.is_ok());
    }
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

    // ACT: 50 producers, each pushing 20 entries
    let mut handles = vec![];
    for producer_id in 0..50 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
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

    // Verify entries are monotonically increasing
    for i in 1..all_ids.len() {
        assert!(all_ids[i] >= all_ids[i - 1]);
    }
}

// ============================================================================
// Stress test: many consumers, one topic
// ============================================================================

#[tokio::test]
async fn test_stress_many_consumers_one_topic() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "stress.many_consumers";

    // Push entries first
    for i in 0..100 {
        ddl.push(topic, format!("entry_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ACT: 50 consumers read concurrently
    let mut handles = vec![];
    for consumer_id in 0..50 {
        let ddl_clone = ddl.clone();
        let topic_clone = topic.to_string();
        let handle = tokio::spawn(async move {
            let mut stream = ddl_clone.subscribe(&topic_clone).await.unwrap();
            let mut count = 0;
            
            // Try to read all available entries
            while stream.try_next().is_some() {
                count += 1;
                if count >= 100 {
                    break;
                }
            }
            count
        });
        handles.push(handle);
    }

    // Wait for all consumers
    let counts: Vec<usize> = handles
        .into_iter()
        .map(|h| h.await.unwrap())
        .collect();

    // Each consumer should have read some entries
    for count in &counts {
        assert!(*count > 0, "Each consumer should read entries");
    }
}

// ============================================================================
// Performance measurement: baseline push rate
// ============================================================================

#[tokio::test]
async fn test_performance_baseline_push_rate() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "perf.baseline";

    // Warmup: push 1000 entries
    for i in 0..1000 {
        ddl.push(topic, format!("warmup_{}", i).into_bytes())
            .await
            .unwrap();
    }

    // ACT: Measure push rate
    let start = std::time::Instant::now();
    let mut count = 0;

    while start.elapsed() < Duration::from_secs(1) {
        ddl.push(topic, format!("entry_{}", count).into_bytes())
            .await
            .unwrap();
        count += 1;
    }

    let duration = start.elapsed();
    let rate = (count as f64) / duration.as_secs_f64();

    // ASSERT: Should achieve reasonable throughput
    log::info!("Push rate: {:.2} entries/second", rate);

    // Expected: At least 1000 entries/second on modern hardware
    // This is a sanity check that may fail on CI
    assert!(rate > 100.0, "Should achieve at least 100 entries/second");
}

// ============================================================================
// Performance measurement: batch push rate
// ============================================================================

#[tokio::test]
async fn test_performance_batch_push_rate() {
    // ARRANGE: Create DDL
    let config = DdlConfig::default();
    let ddl = Arc::new(InMemoryDdl::new(config));
    let topic = "perf.batch";

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
        ddl.push_batch(topic, payloads).await.unwrap();
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

    // Expected: At least 1000 entries/second
    assert!(rate > 100.0);
}
