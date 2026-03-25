//! Comprehensive tests for the queue system
//!
//! This file contains tests for SPMC lock-free queues and their integration
//! with source implementations.

use ddl::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use ddl::traits::queue::QueueTrait;

// ============================================================================
// Test SPMC queue basic operations
// ============================================================================

#[test]
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

// ============================================================================
// Test FunctionSource pause/resume (basic - actual implementation in function_source_test.rs)
// ============================================================================

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
    let consumer1 = queue.consumer();
    let consumer2 = queue.consumer();
    let consumer3 = queue.consumer();

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
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, 100);
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
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ASSERT: Empty queue has size 0
    assert_eq!(queue.get_size(), 0);

    // ACT: Push entries directly
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

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
// ============================================================================
// Test consumer read behavior
// ============================================================================

#[test]
fn test_consumer_has_data() {
    // ARRANGE: Create queue
    let mut queue: SimpleQueue<i32, 16> = SimpleQueue::new();

    // ASSERT: Empty queue has no data
    assert!(queue.is_empty());

    // ACT: Push entry
    queue.push(42).unwrap();

    // ASSERT: Queue has data
    assert!(!queue.is_empty());
    
    // ACT: Create consumer after push
    let consumer = queue.consumer();
    // ASSERT: Consumer now has data
    assert!(consumer.has_data());
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
