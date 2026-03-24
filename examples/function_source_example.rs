//! Example showing how to use FunctionSource with pause/resume/remove functionality

use ddl::queue::interval::IntervalConfig;
use ddl::queue::source::FunctionSource;
use ddl::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use ddl::queue::source::QueueSource;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("FunctionSource Example");
    println!("=====================");
    
    // Create a queue for storing the data
    let queue = Arc::new(RwLock::new(SimpleQueue::<f64, 1024>::new()));
    
    // Create a function that generates data (e.g., simulated CPU usage)
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = counter.clone();
    
    let source = FunctionSource::new(move || {
        let count = counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Simulate some varying data - sine wave pattern
        (count as f64 * 0.1).sin().abs() * 100.0
    });
    
    // Create interval configuration
    let interval_config = IntervalConfig::Constant(100); // 100ms interval
    
    // Start the source - this will begin generating data in the background
    println!("Starting function source...");
    source.start(queue.clone(), interval_config, None);
    
    // Let it run for a bit and observe the data
    println!("Running for 500ms...");
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Pause the source
    println!("\nPausing the source...");
    source.pause();
    
    // Record current count
    let paused_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    
    // Let it sit paused for a while
    println!("Waiting 500ms while paused...");
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Check count hasn't changed much (accounting for potential race conditions)
    let after_pause_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    println!("Counter before pause: {}, after pause: {}", paused_count, after_pause_count);
    
    // Resume the source
    println!("\nResuming the source...");
    source.resume();
    
    // Let it run again
    println!("Running for another 500ms...");
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Remove/stop the source
    println!("\nRemoving/stopping the source...");
    source.remove();
    
    // Let it sit stopped for a while
    let stopped_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    println!("Waiting 500ms after removal...");
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Check that it didn't increment after being removed
    let final_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    println!("Counter after removal: {}, final count: {}", stopped_count, final_count);
    
    println!("\nFunctionSource example completed successfully!");
    println!("Note: To see actual queue data, you would create a consumer and read from it.");
}