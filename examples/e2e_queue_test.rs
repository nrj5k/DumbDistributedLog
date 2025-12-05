//! End-to-End Queue Integration Test
//!
//! Tests the complete queue system with real data flow.

use autoqueues::{Queue, queue::SimpleQueue};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 End-to-End Queue Integration Test");
    println!("=====================================\n");

    // Test 1: Basic Queue Operations
    println!("📋 Test 1: Basic Queue Operations");
    test_basic_queue_operations().await?;

    // Test 2: Generic Data Types
    println!("\n📋 Test 2: Generic Data Types");
    test_generic_data_types().await?;

    // Test 3: Performance Test
    println!("\n📋 Test 3: Performance Test");
    test_performance().await?;

    // Test 4: Error Handling
    println!("\n📋 Test 4: Error Handling");
    test_error_handling().await?;

    println!("\n✅ All end-to-end tests passed!");
    Ok(())
}

async fn test_basic_queue_operations() -> Result<(), Box<dyn std::error::Error>> {
    let mut queue: SimpleQueue<i32> = SimpleQueue::new();

    // Test publishing
    for i in 1..=10 {
        queue.publish(i)?;
        println!("   Published: {}", i);
    }

    // Test retrieval
    if let Some((timestamp, latest)) = queue.get_latest() {
        println!("   Latest: {} at timestamp {}", latest, timestamp);
        assert_eq!(latest, 10);
    }

    let recent = queue.get_latest_n(5);
    println!("   Recent 5: {:?}", recent);
    assert_eq!(recent, vec![10, 9, 8, 7, 6]);

    println!("   ✅ Basic operations work correctly");
    Ok(())
}

async fn test_generic_data_types() -> Result<(), Box<dyn std::error::Error>> {
    // Test with different data types
    let mut string_queue: SimpleQueue<String> = SimpleQueue::new();
    let mut struct_queue: SimpleQueue<TestStruct> = SimpleQueue::new();

    string_queue.publish("Hello World".to_string())?;
    string_queue.publish("AutoQueues".to_string())?;

    #[derive(Clone, Debug)]
    struct TestStruct {
        id: u32,
        name: String,
        value: f64,
    }

    struct_queue.publish(TestStruct {
        id: 1,
        name: "Test".to_string(),
        value: 42.5,
    })?;

    // Verify retrieval
    if let Some((_, latest_string)) = string_queue.get_latest() {
        println!("   Latest string: {}", latest_string);
        assert_eq!(latest_string, "AutoQueues");
    }

    if let Some((_, latest_struct)) = struct_queue.get_latest() {
        println!("   Latest struct: {:?}", latest_struct);
        assert_eq!(latest_struct.id, 1);
    }

    println!("   ✅ Generic data types work correctly");
    Ok(())
}

async fn test_performance() -> Result<(), Box<dyn std::error::Error>> {
    let mut queue: SimpleQueue<i32> = SimpleQueue::new();

    // Performance test: Publish 1000 items
    let start = std::time::Instant::now();
    for i in 1..=1000 {
        queue.publish(i)?;
    }
    let publish_time = start.elapsed();

    // Performance test: Retrieve latest
    let start = std::time::Instant::now();
    let _ = queue.get_latest();
    let retrieve_time = start.elapsed();

    // Performance test: Retrieve N items
    let start = std::time::Instant::now();
    let _ = queue.get_latest_n(100);
    let retrieve_n_time = start.elapsed();

    println!("   Published 1000 items in {:?}", publish_time);
    println!("   Retrieved latest in {:?}", retrieve_time);
    println!("   Retrieved 100 items in {:?}", retrieve_n_time);

    // Performance assertions
    assert!(publish_time.as_millis() < 100); // Should be fast
    assert!(retrieve_time.as_micros() < 1000); // Should be very fast
    assert!(retrieve_n_time.as_micros() < 10000); // Should be fast

    println!("   ✅ Performance tests passed");
    Ok(())
}

async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    let queue: SimpleQueue<i32> = SimpleQueue::new();

    // Test empty queue
    assert_eq!(queue.get_latest(), None);
    assert_eq!(queue.get_latest_n(5), Vec::<i32>::new());

    // Test server lifecycle
    let server_handle = queue.start_server()?;
    println!("   Server started successfully");

    // Shutdown server
    tokio::time::sleep(Duration::from_millis(100)).await;
    server_handle.shutdown().await?;
    println!("   Server shutdown successfully");

    println!("   ✅ Error handling works correctly");
    Ok(())
}
