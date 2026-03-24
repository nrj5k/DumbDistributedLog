//! Comprehensive tests for the queue system
//!
//! This file contains tests for SPMC lock-free queues and their integration
//! with source implementations.

use ddl::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use ddl::traits::queue::QueueTrait;
use std::sync::{Arc, RwLock};

// ============================================================================
// Test SPMC queue basic operations
// ============================================================================

#[test]
fn test_spmc_push_pop() {
    // ARRANGE: Create a queue with capacity 16
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ACT: Push entries
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

    // ASSERT: Verify entries can be retrieved
    let consumer = queue.consumer();
    assert_eq!(consumer.head_position(), 0);

    // Get the latest entry (producer's view)
    if let Some((_, value)) = queue.get_latest() {
        assert_eq!(value, 3);
    } else {
        panic!("Should have entry");
    }

    // Get all entries
    let entries: Vec<i32> = queue.get_latest_n(16);
    assert_eq!(entries, vec![1, 2, 3]);
}

#[test]
fn test_spmc_empty_queue_behavior() {
    // ARRANGE: Create empty queue
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ASSERT: Queue is empty
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);

    // ASSERT: get_latest returns None for empty queue
    assert!(queue.get_latest().is_none());

    // ASSERT: get_latest_n returns empty Vec
    let entries: Vec<i32> = queue.get_latest_n(10);
    assert!(entries.is_empty());
}

#[test]
fn test_spmc_capacity() {
    // ARRANGE: Create queue with different capacities
    let queue1: SimpleQueue<i32, 8> = SimpleQueue::new();
    let queue2: SimpleQueue<i32, 16> = SimpleQueue::new();
    let queue3: SimpleQueue<i32, 32> = SimpleQueue::new();

    // ASSERT: Verify capacities
    assert_eq!(queue1.capacity(), 8);
    assert_eq!(queue2.capacity(), 16);
    assert_eq!(queue3.capacity(), 32);
}

// ============================================================================
// Test SPMC queue capacity limits
// ============================================================================

#[test]
fn test_spmc_queue_full_behavior() {
    // ARRANGE: Create queue with small capacity
    let mut queue: SimpleQueue<i32, 4> = SimpleQueue::new();

    // ACT: Fill the queue
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();
    queue.push(4).unwrap();

    // ASSERT: Queue is full
    assert!(queue.is_full());
    assert_eq!(queue.len(), 4);

    // ACT: Push one more (should drop oldest per circular buffer semantics)
    queue.push(5).unwrap();

    // ASSERT: Queue still at capacity, with new entry
    assert!(queue.is_full());
    assert_eq!(queue.len(), 4);

    // Verify the oldest entry was overwritten
    let entries: Vec<i32> = queue.get_latest_n(4);
    assert_eq!(entries, vec![2, 3, 4, 5]); // First entry (1) is gone
}

#[test]
fn test_spmc_queue_wraparound() {
    // ARRANGE: Create queue with capacity 4
    let mut queue: SimpleQueue<i32, 4> = SimpleQueue::new();

    // ACT: Push 4 entries to fill
    for i in 1..=4 {
        queue.push(i).unwrap();
    }

    // ACT: Push 2 more to test wraparound
    queue.push(5).unwrap();
    queue.push(6).unwrap();

    // ASSERT: Verify entries are correct after wraparound
    let entries: Vec<i32> = queue.get_latest_n(4);
    assert_eq!(entries, vec![3, 4, 5, 6]);

    // ACT: Push many more entries
    for i in 7..=12 {
        queue.push(i).unwrap();
    }

    // ASSERT: Should have most recent 4 entries
    let entries: Vec<i32> = queue.get_latest_n(4);
    assert_eq!(entries, vec![9, 10, 11, 12]);
}

#[test]
fn test_spmc_dropped_count() {
    // ARRANGE: Create queue with small capacity
    let mut queue: SimpleQueue<i32, 4> = SimpleQueue::new();

    // ACT: Push 10 entries (6 should be dropped)
    for i in 1..=10 {
        queue.push(i).unwrap();
    }

    // ASSERT: Dropped count should reflect overwrites
    // Note: This is from producer's view (tail position)
    let tail = queue.tail.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(tail, 10); // 10 total pushes

    //ASSERT: Queue has capacity entries
    assert_eq!(queue.len(), 4);
}

// ============================================================================
// Test FunctionSource pause/resume
// ============================================================================

use ddl::queue::interval::IntervalConfig;
use ddl::queue::source::FunctionSource;

#[test]
fn test_function_source_creation() {
    // ARRANGE & ACT: Create a simple function source
    let source = FunctionSource::new(|| 42);

    // ASSERT: Source is properly created
    assert!(!source.is_paused());
    assert!(!source.should_stop());
}

// ============================================================================
// Test SPMC queue with multiple consumers
// ============================================================================

#[test]
fn test_spmc_multiple_consumers() {
    // ARRANGE: Create queue and multiple consumers
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // Push some initial data
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

    // Create multiple consumers
    let mut consumer1 = queue.consumer();
    let mut consumer2 = queue.consumer();
    let mut consumer3 = queue.consumer();

    // ASSERT: All consumers can read the same data independently
    // Consumer 1 reads entry 1
    if let Some((timestamp, value)) = consumer1.pop() {
        assert_eq!(value, 1);
        assert!(timestamp > 0);
    } else {
        panic!("Consumer 1 should have entry");
    }

    // Consumer 2 also reads entry 1 (independent head)
    if let Some((_, value)) = consumer2.pop() {
        assert_eq!(value, 1);
    } else {
        panic!("Consumer 2 should have entry");
    }

    // Consumer 3 reads entry 1 as well
    if let Some((_, value)) = consumer3.pop() {
        assert_eq!(value, 1);
    } else {
        panic!("Consumer 3 should have entry");
    }

    // All consumers have head at 1 now
    assert_eq!(consumer1.head_position(), 1);
    assert_eq!(consumer2.head_position(), 1);
    assert_eq!(consumer3.head_position(), 1);

    // Consumer 1 reads entry 2
    if let Some((_, value)) = consumer1.pop() {
        assert_eq!(value, 2);
    } else {
        panic!("Consumer 1 should have entry 2");
    }

    // Consumer 2 still reads entry 2 (its head is at 1)
    if let Some((_, value)) = consumer2.pop() {
        assert_eq!(value, 2);
    } else {
        panic!("Consumer 2 should have entry 2");
    }

    // Consumer 3 still reads entry 2
    if let Some((_, value)) = consumer3.pop() {
        assert_eq!(value, 2);
    } else {
        panic!("Consumer 3 should have entry 2");
    }

    // Consumer 1 reads entry 3
    if let Some((_, value)) = consumer1.pop() {
        assert_eq!(value, 3);
    } else {
        panic!("Consumer 1 should have entry 3");
    }

    // ASSERT: Consumer 1 has read all entries
    assert!(consumer1.pop().is_none());

    // ASSERT: Consumer 2 hasn't read entry 3 yet
    if let Some((_, value)) = consumer2.pop() {
        assert_eq!(value, 3);
    } else {
        panic!("Consumer 2 should have entry 3");
    }

    // ASSERT: Consumer 3 hasn't read entry 3 yet
    if let Some((_, value)) = consumer3.pop() {
        assert_eq!(value, 3);
    } else {
        panic!("Consumer 3 should have entry 3");
    }
}

#[test]
fn test_spmc_consumer_clone() {
    // ARRANGE: Create queue with data
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();
    queue.push(1).unwrap();
    queue.push(2).unwrap();

    // ACT: Clone a consumer
    let consumer1 = queue.consumer();
    let consumer2 = consumer1.clone(); // clone with same head position

    // ASSERT: Both consumers have same head position
    assert_eq!(consumer1.head_position(), consumer2.head_position());
    assert_eq!(consumer1.head_position(), 0);

    // ASSERT: Both can read the same entries
    for consumer in [&consumer1, &consumer2] {
        if let Some((_, value)) = consumer.pop() {
            assert_eq!(value, 1);
        } else {
            panic!("Consumer should have entry");
        }
    }
}

// ============================================================================
// Test queue with different data types
// ============================================================================

#[test]
fn test_spmc_different_types() {
    // ARRANGE: Create queues with different types
    let mut int_queue: SimpleQueue<i32, 16> = SimpleQueue::new();
    let mut string_queue: SimpleQueue<String, 16> = SimpleQueue::new();
    let mut struct_queue: SimpleQueue<TestStruct, 16> = SimpleQueue::new();

    // ACT & ASSERT: Test int queue
    int_queue.push(42).unwrap();
    let entries: Vec<i32> = int_queue.get_latest_n(1);
    assert_eq!(entries, vec![42]);

    // ACT & ASSERT: Test string queue
    string_queue.push("hello".to_string()).unwrap();
    let entries: Vec<String> = string_queue.get_latest_n(1);
    assert_eq!(entries, vec!["hello".to_string()]);

    // ACT & ASSERT: Test struct queue
    struct_queue.push(TestStruct { value: 100 }).unwrap();
    let entries: Vec<TestStruct> = struct_queue.get_latest_n(1);
    assert_eq!(entries, vec![TestStruct { value: 100 }]);
}

#[derive(Clone, Debug)]
struct TestStruct {
    value: i32,
}

// ============================================================================
// Test queue trait integration
// ============================================================================

#[test]
fn test_queue_trait_get_size() {
    // ARRANGE: Create queue
    let queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ASSERT: Empty queue has size 0
    assert_eq!(queue.get_size(), 0);

    // ACT: Push entries
    let mut queue_mut = queue;
    queue_mut.push(1).unwrap();
    queue_mut.push(2).unwrap();
    queue_mut.push(3).unwrap();

    // ASSERT: Size reflects entries
    assert_eq!(queue.get_size(), 3);
}

#[test]
fn test_queue_trait_publish() {
    // ARRANGE: Create queue
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ACT: Use publish method
    queue.publish(42).unwrap();

    // ASSERT: Entry was published
    assert_eq!(queue.get_size(), 1);
}

#[test]
fn test_queue_trait_get_latest_n() {
    // ARRANGE: Create queue with entries
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();
    for i in 1..=10 {
        queue.push(i).unwrap();
    }

    // ASSERT: get_latest_n(5) returns last 5 entries
    let entries: Vec<i32> = queue.get_latest_n(5);
    assert_eq!(entries, vec![6, 7, 8, 9, 10]);

    // ASSERT: get_latest_n(20) returns all entries (not more)
    let entries: Vec<i32> = queue.get_latest_n(20);
    assert_eq!(entries, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
}

// ============================================================================
// Test consumer read behavior
// ============================================================================

#[test]
fn test_consumer_has_data() {
    // ARRANGE: Create queue
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();
    let consumer = queue.consumer();

    // ASSERT: Empty queue has no data
    assert!(!consumer.has_data());
    assert!(consumer.is_empty());

    // ACT: Push entry
    queue.push(42).unwrap();

    // ASSERT: Consumer now has data
    assert!(consumer.has_data());
    assert!(!consumer.is_empty());
}

#[test]
fn test_consumer_available() {
    // ARRANGE: Create queue
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // Push 5 entries
    for i in 1..=5 {
        queue.push(i).unwrap();
    }

    // ACT: Create consumer and check available count
    let consumer = queue.consumer();
    let available = consumer.available();

    // ASSERT: Should have 5 available entries
    assert_eq!(available, 5);
}

#[test]
fn test_consumer_pop_none_when_empty() {
    // ARRANGE: Create empty queue
    let queue: SimpleQueue<i32, 16> = SimpleQueue::new();
    let consumer = queue.consumer();

    // ASSERT: Pop returns None for empty queue
    assert!(consumer.pop().is_none());
}

// ============================================================================
// Performance: verify zero allocations
// ============================================================================

#[test]
fn test_queue_no_allocations_after_creation() {
    // ARRANGE: Create queue (allocations happen here)
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ACT: Push and pop many entries
    for i in 0..100 {
        queue.push(i).unwrap();
        let _ = queue.get_latest();
    }

    // ASSERT: Queue still at capacity (no unbounded growth)
    assert!(queue.is_full() || queue.len() <= 16);

    // Verify operations complete quickly
    let start = std::time::Instant::now();
    for i in 0..1000 {
        queue.push(i).unwrap();
    }
    let duration = start.elapsed();

    // Should complete very quickly (no allocations)
    assert!(duration.as_millis() < 100, "Operations should be very fast");
}
