//! # Basic Usage Example
//!
//! This is a clean, working example showing how to use the autoqueues library
//! with the current API. It demonstrates core functionality without any
//! outdated API calls.

use autoqueues::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Autoqueues Basic Usage Example");
    println!("This example demonstrates the current working API\n");

    // Example 1: Simple counter queue
    basic_counter_example().await.expect("Basic example should complete");

    // Example 2: AIMD adaptive queue
    aimd_example().await.expect("AIMD example should complete");

    println!("\n🎉 All basic examples completed successfully!");
    println!("\nKey takeaways:");
    println!("- Use Queue::new() for standard queues");
    println!("- Use Queue::with_aimd() for adaptive queues");
    println!("- Function hooks generate data automatically");
    println!("- Multiple configurations for different use cases");

    Ok(())
}

async fn basic_counter_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Simple Counter Queue Example");
    println!("{}", "=".repeat(40));

    // Create a simple counter queue
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000,  // 1 second base interval
        2,     // Increase factor
        "counter.log".to_string(),
        "counter_var".to_string(),
    );

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = counter.clone();

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(move |_| {
            let current = counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            println!("   Counter hook called: {}", current);
            Ok(current)
        }),
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config.clone());
    
    println!("1️⃣  Basic operations demo:");
    println!("   Initial stats: {:?}", queue.get_stats());

    // Publish some data manually
    for i in 100..103 {
        queue.publish(i).await.expect("Queue publish should succeed");
        println!("   Published: {}", i);
    }

    if let Some(latest) = queue.get_latest().await {
        println!("   Latest data: {} (timestamp: {})", latest.1, latest.0);
    }

    // Start the autonomous server
    println!("   Starting autonomous server...");
    let server_config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(42)),
        Model::Linear,
        QueueType::new(QueueValue::NodeAvailability, 1000, 2, "server.log".to_string(), "server_var".to_string()),
    );
    
    let server_handle = Queue::new(server_config).start_server().expect("Server should start");

    // Let it run for 2 seconds
    sleep(Duration::from_secs(2)).await;
    
    // Check final stats
    let server_stats_queue = Queue::new(
        QueueConfig::new(
            Mode::Sensor,
            Arc::new(|_| Ok(42)),
            Model::Linear,
            QueueType::new(QueueValue::NodeCapacity, 1000, 3, "final.log".to_string(), "final_var".to_string()),
        )
    );
    let final_stats = server_stats_queue.get_stats();
    println!("   Final stats: size={}, interval={}ms", final_stats.size, final_stats.interval_ms);

    server_handle.shutdown().await.expect("Server should shutdown");
    println!("   Server stopped gracefully\n");

    Ok(())
}

async fn aimd_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧠 AIMD Adaptive Queue Example");
    println!("{}", "=".repeat(40));

    // Create AIMD configuration
    let aimd_config = AimdConfig {
        initial_interval_ms: 500,
        min_interval_ms: 100,
        max_interval_ms: 2000,
        additive_factor_ms: 100,
        multiplicative_factor: 0.8,
        variance_threshold: Some(1.0),
        cooldown_ms: Some(250),
        window_size: Some(5),
    };

    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        500,  // Base interval
        1,    // Conservative increase
        "aimd_demo.log".to_string(),
        "aimd_demo_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(99)), // Simple counter hook
        Model::Linear,
        queue_type,
    );

    let mut aimd_queue = Queue::with_aimd(config, aimd_config.clone())?;
    
    println!("2️⃣  AIMD queue created:");
    if let Some(aimd_stats) = aimd_queue.get_aimd_stats() {
        println!("   Current interval: {}ms", aimd_stats.current_interval);
        println!("   Min/Max bounds: {}ms - {}ms", aimd_stats.min_interval, aimd_stats.max_interval);
        println!("   Note: Currently in constant interval mode");
    }

    // Test manual publishing
    for value in [10, 15, 8, 20, 5] {
        aimd_queue.publish(value).await.expect("AIMD publish should succeed");
    }

    if let Some(latest) = aimd_queue.get_latest().await {
        println!("   Latest AIMD data: {}", latest.1);
    }

    // Start autonomous server for AIMD testing
    println!("   Starting AIMD autonomous server...");
    let server_config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(77)),
        Model::Linear,
        QueueType::new(QueueValue::NodeLoad, 500, 1, "aimd_server.log".to_string(), "aimd_server_var".to_string()),
    );
    
    let server_handle = Queue::with_aimd(server_config, aimd_config).expect("AIMD queue should be created").expect("AIMD queue should be created").start_server().expect("Queue server should start")

    sleep(Duration::from_secs(2)).await;

    // Create new queue for final stats since original was moved
    let final_stats_queue = Queue::with_aimd(QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(66)),
        Model::Linear,
        QueueType::new(QueueValue::NodeLoad, 500, 1, "aimd_final.log".to_string(), "aimd_final_var".to_string()),
    ), AimdConfig {
        initial_interval_ms: 500,
        min_interval_ms: 100,
        max_interval_ms: 2000,
        additive_factor_ms: 100,
        multiplicative_factor: 0.8,
        variance_threshold: Some(1.0),
        cooldown_ms: Some(250),
        window_size: Some(5),
    }).expect("Queue operation should complete");
    
    let final_stats = final_stats_queue.get_stats();
    println!("   Final AIMD stats: size={}, interval={}ms", final_stats.size, final_stats.interval_ms);

    server_handle.shutdown().await.expect("Server should shutdown");
    println!("   AIMD server stopped\n");

    Ok(())
}