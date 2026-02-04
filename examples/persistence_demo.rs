//! Persistence Demo
//!
//! Demonstrates the persistence functionality in AutoQueues.

use autoqueues::autoqueues::AutoQueues;
use autoqueues::config::Config;
use autoqueues::queue::persistence::PersistenceConfig;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting AutoQueues with persistence demo...");
    
    // Create a config with persistence enabled
    let config = Config::builder()
        .capacity(1000)
        .default_interval(100) // 100ms
        .persistence(PersistenceConfig {
            data_dir: PathBuf::from("demo_data"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            flush_interval_ms: 500,          // 500ms
            include_timestamp: true,
            compress_old: false,
        })
        .build();
    
    // Create AutoQueues instance
    let autoqueues = AutoQueues::new(config);
    
    // Add a queue with persistence enabled
    autoqueues
        .add_queue_fn_with_persistence::<f64, _>("cpu_monitor", || {
            // Simulate CPU usage data
            let cpu_usage = 20.0 + (rand::random::<f64>() * 60.0);
            println!("Generated CPU usage: {:.2}%", cpu_usage);
            cpu_usage
        }, autoqueues::queue::interval::IntervalConfig::default())?;
    
    // Add another queue with expression and persistence
    autoqueues
        .add_queue_expr_with_persistence(
            "cpu_alert", 
            "local.cpu_monitor", 
            "cpu_monitor", 
            true, 
            Some(100)
        )?;
    
    // Start the queues
    autoqueues.start();
    
    println!("Queues started. Generating data for 5 seconds...");
    
    // Let it run for a bit to generate some data
    sleep(Duration::from_secs(5)).await;
    
    // Enable persistence for an existing queue
    autoqueues.enable_persistence("cpu_alert")?;
    
    println!("Enabled persistence for cpu_alert queue");
    
    // Let it run a bit more
    sleep(Duration::from_secs(2)).await;
    
    println!("Demo completed. Check the demo_data directory for log files.");
    
    // Proper shutdown to flush all data
    autoqueues.shutdown();
    
    println!("Press Ctrl+C to stop the demo.");
    
    // Keep running
    loop {
        sleep(Duration::from_secs(1)).await;
    }
}