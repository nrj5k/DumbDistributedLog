//! # Autoqueues Modern Example
//!
//! This is a complete, modern example demonstrating the current autoqueues API
//! with realistic system monitoring scenarios.

use autoqueues::{Queue, QueueConfig, QueueType, QueueError, QueueOperations};
use autoqueues::{QueueValue, Mode, Model, AimdConfig};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};

/// System monitoring data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemMetric {
    cpu_usage: f64,
    memory_usage: f64,
    disk_usage: f64,
    network_io: u64,
    timestamp: u64,
}

/// Generated realistic system metrics
fn generate_system_metrics() -> SystemMetric {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    timestamp.hash(&mut hasher);
    let hash = hasher.finish();
    
    SystemMetric {
        cpu_usage: (hash % 80) as f64,           // 0-80% CPU usage
        memory_usage: (hash % 90) as f64 + 10.0, // 10-100% memory usage
        disk_usage: (hash % 85) as f64 + 15.0,   // 15-100% disk usage
        network_io: hash % 1000000,              // Network I/O operations
        timestamp,
    }
}

/// Processes system metrics history
fn process_metrics_history(history: Vec<SystemMetric>) -> Result<SystemMetric, QueueError> {
    // Simple processor - can add trend analysis here
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| QueueError::Other("Time calculation error".to_string()))?
        .as_secs();
    
    let mut metric = generate_system_metrics();
    metric.timestamp = current_time;
    
    // If we have history, we could calculate trends
    if !history.is_empty() {
        let avg_cpu: f64 = history.iter().take(5).map(|m| m.cpu_usage).sum::<f64>() / history.len().min(5) as f64;
        if metric.cpu_usage > avg_cpu * 1.5 {
            println!("⚠️  CPU spike detected: {}% vs average {}%", metric.cpu_usage, avg_cpu);
        }
    }
    
    Ok(metric)
}

/// Demonstrates basic queue operations
async fn basic_queue_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Basic Queue Operations Demo");
    println!("{}", "=".repeat(35));

    // Create a standard monitoring queue
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000,  // 1 second base interval
        2,     // Increase factor
        "system_monitor.log".to_string(),
        "sys_metrics".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_metrics_history),
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config);  
    
    // Show initial state
    let stats = queue.get_stats();
    println!("🔄 Initial queue stats:");
    println!("   Size: {}/{} (capacity)", stats.size, stats.capacity);
    println!("   Mode: {:?}, Model: {:?}", stats.mode, stats.model);
    println!("   Interval: {}ms", stats.interval_ms);

    // Demonstrate manual publishing
    let manual_metric = SystemMetric {
  cpu_usage: 75.5,
   memory_usage: 82.3,
        disk_usage: 68.7,
        network_io: 125000,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    };
    
    queue.publish(manual_metric.clone()).await?;
    println!("\n✅ Published manual metric:");
    println!("   CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%", 
 manual_metric.cpu_usage, manual_metric.memory_usage, manual_metric.disk_usage);

    // Show the data we have without starting server
    if let Some(latest) = queue.get_latest().await {
        let data = latest.1;
        println!("\n📊 Latest system metric (timestamp: {}):", latest.0);
        println!("   CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%",
 data.cpu_usage, data.memory_usage, data.disk_usage);
    }

    // Now demonstrate autonomous server
    println!("\n🔥 Starting autonomous server for 3 seconds...");
  let server_handle = queue.start_server()?;
    
    sleep(Duration::from_secs(3)).await;
    
    // Server stops, queue is now moved/stopped
  server_handle.shutdown().await?;
    
    println!("✅ Basic queue demonstration completed");

    Ok(())
}

/// Shows AIMD functionality (constant interval mode)
async fn aimd_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧠 AIMD Adaptive Queue Demo");
    println!("{}", "=".repeat(30));

    // Create AIMD configuration
    let aimd_config = AimdConfig {
        initial_interval_ms: 500,   // Start with 500ms
        min_interval_ms: 100,       // Minimum 100ms
        max_interval_ms: 2000,      // Maximum 2000ms
        additive_factor_ms: 50,     // Add 50ms when increasing
        multiplicative_factor: 0.9, // Multiply by 0.9 when decreasing
        variance_threshold: Some(0.5),
        cooldown_ms: Some(250),
        window_size: Some(10),
    };

    // Create queue type for AIMD
    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        500,  // Base interval for AIMD
        1,    // Conservative increase factor
        "aimd_monitor.log".to_string(),
        "adaptive_metrics".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_metrics_history),
        Model::Linear,
        queue_type,
    );

    let aimd_queue = Queue::with_aimd(config, aimd_config.clone())?;
    
    println!("🎯 AIMD queue created with adaptive intervals:");
    println!("   Constant interval: {}ms (average of min {} + max {})", 
             (100 + 2000) / 2, 100, 2000);
    println!("   Note: Currently in constant interval mode (variance calculation disabled)");

    // Check AIMD stats before starting server
    if let Some(aimd_stats) = aimd_queue.get_aimd_stats() {
        println!("📊 Initial AIMD stats:");
        println!("   Current interval: {}ms", aimd_stats.current_interval);
        println!("   Min/Max bounds: {}ms - {}ms", 
                aimd_stats.min_interval, aimd_stats.max_interval);
    }

    // Let server run briefly without moving the queue
    println!("\n🔥 Starting autonomous server for 4 seconds...");
    let server_handle = aimd_queue.start_server()?;
    
    // Server runs independently
    sleep(Duration::from_secs(4)).await;
    
    // Stop server
    server_handle.shutdown().await?;
    
    // Create a fresh queue to demonstrate final stats
    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        500,
        1,
        "aimd_final.log".to_string(),
        "final_metrics".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_metrics_history),
        Model::Linear,
        queue_type,
    );

    let final_queue = Queue::with_aimd(config, aimd_config)?;
    
    let final_stats = final_queue.get_stats();
    if let Some(aimd_stats) = final_queue.get_aimd_stats() {
        println!("\n📈 Final AIMD demonstration:");
        println!("   Queue capacity simulation: {}/{}", final_stats.size, final_stats.capacity);
        println!("   Current interval: {}ms (constant)", aimd_stats.current_interval);
    }
    
    println!("✅ AIMD demonstration completed");

    Ok(())
}

/// Demonstrates various queue configurations
async fn configurations_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔧 Configuration Showcase");
    println!("{}", "=".repeat(30));

    let scenarios = vec![
        ("High-frequency monitoring", QueueType::new(
            QueueValue::NodeAvailability,
            100,  // 100ms intervals
            2,    // Moderate increase
            "high_freq.log".to_string(),
      "high_freq_metrics".to_string(),
        )),
        ("Standard production monitoring", QueueType::new(
   QueueValue::NodeLoad,
            1000, // 1 second intervals
            2,    // Conservative increase
            "prod_monitor.log".to_string(),
            "prod_metrics".to_string(),
      )),
        ("Low-frequency archival", QueueType::new(
 QueueValue::NodeCapacity,
       10000, // 10 second intervals
            5,     // Very conservative
            "archival.log".to_string(),
            "archival_metrics".to_string(),
     )),
    ];

    for (name, queue_type) in scenarios {
        let config = QueueConfig::new(
            Mode::Sensor,
Arc::new(process_metrics_history),
            Model::Linear,
   queue_type,
        );

        let queue = Queue::new(config.clone());
        let server_handle = queue.start_server()?;
        
        // Let it run briefly
        sleep(Duration::from_millis(300)).await;
        
        // Stop server and create fresh queue for stats
        server_handle.shutdown().await?;
        
        let fresh_queue = Queue::new(config);
        let stats = fresh_queue.get_stats();
        println!("{}: interval={}ms, size={}/{}", 
          name, stats.interval_ms, stats.size, stats.capacity);
    }

    println!("✅ Configuration showcase completed");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Autoqueues Modern Examples");
    println!("Demonstrating current API and real-world applications\n");

    // Run all demonstrations
    basic_queue_demo().await?;
    aimd_demo().await?;
    configurations_demo().await?;

    println!("\n🎉 All modern examples completed successfully!");
 println!("\nKey findings:");
    println!("- Current API uses QueueType/QueueConfig for configuration");
    println!("- AIMD operates in constant interval mode (min+max average)");
    println!("- Function hooks process historical data before queuing");
    println!("- Flexible configuration options for different use cases");
    println!("- Easy integration with async Rust applications");
    
    Ok(())
}