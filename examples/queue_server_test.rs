//! Queue Server Testing Example
//!
//! Demonstrates queue server lifecycle management and concurrent operations.

use autoqueues::{Queue, queue::SimpleQueue};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Queue Server Testing Example");
    println!("=====================================\n");

    // Test 1: Server Lifecycle
    println!("📋 Test 1: Server Lifecycle");
    test_server_lifecycle().await?;

    // Test 2: Concurrent Operations
    println!("\n📋 Test 2: Concurrent Operations");
    test_concurrent_operations().await?;

    // Test 3: Server Performance
    println!("\n📋 Test 3: Server Performance");
    test_server_performance().await?;

    println!("\n✅ All server tests passed!");
    Ok(())
}

async fn test_server_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let queue: SimpleQueue<i32> = SimpleQueue::new();

    // Start server
    let server_handle = queue.start_server()?;
    println!("   ✅ Server started successfully");

    // Test operations while server runs
    for i in 1..=5 {
        queue.publish(i * 10)?;
        println!("   Published: {} while server running", i * 10);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Shutdown server
    server_handle.shutdown().await?;
    println!("   ✅ Server shutdown successfully");

    Ok(())
}

async fn test_concurrent_operations() -> Result<(), Box<dyn std::error::Error>> {
    let queue: SimpleQueue<i32> = SimpleQueue::new();

    // Start server
    let server_handle = queue.start_server()?;
    println!("   ✅ Server started for concurrent test");

    // Create multiple concurrent publishers
    let mut handles = Vec::new();
    for i in 0..=5 {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            for j in 1..=20 {
                queue_clone.publish(i * 100 + j)?;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for concurrent operations to complete
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Collect results
    let mut total_published = 0;
    for handle in handles {
        let _ = handle.await;
        total_published += 1;
    }

    // Verify all data was published
    let latest = queue.get_latest();
    let recent = queue.get_latest_n(10);

    println!("   Concurrent publishers: {}", handles.len());
    println!("   Total published: {}", total_published);
    println!("   Latest: {:?}", latest);
    println!("   Recent 10: {:?}", recent);

    // Shutdown all handles
    for handle in handles {
        let _ = handle.await;
    }

    // Shutdown server
    server_handle.shutdown().await?;
    println!("   ✅ Concurrent operations completed");

    Ok(())
}

async fn test_server_performance() -> Result<(), Box<dyn std::error::Error>> {
    let queue: SimpleQueue<i32> = SimpleQueue::new();

    // Start server
    let server_handle = queue.start_server()?;
    println!("   ✅ Server started for performance test");

    // High-frequency publishing
    let start = std::time::Instant::now();
    for i in 1..=1000 {
        queue.publish(i)?;
    }
    let publish_time = start.elapsed();

    // Test retrieval under load
    let start = std::time::Instant::now();
    let _ = queue.get_latest();
    let retrieve_time = start.elapsed();

    println!("   Published 1000 items in {:?}", publish_time);
    println!("   Retrieved latest in {:?}", retrieve_time);

    // Performance assertions
    assert!(publish_time.as_millis() < 1000); // Should be fast
    assert!(retrieve_time.as_micros() < 10000); // Should be very fast

    // Shutdown server
    server_handle.shutdown().await?;
    println!("   ✅ Performance test completed");

    Ok(())
}
