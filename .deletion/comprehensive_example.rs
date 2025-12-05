//! # Autoqueues Comprehensive Example
//!
//! This is a complete, modern example demonstrating all features of the autoqueues library
//! with real-world scenarios including IoT sensors, financial data, and system monitoring.

use autoqueues::{Queue, QueueConfig, QueueType, QueueError, QueueOperations};
use autoqueues::{QueueValue, Mode, Model};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

/// System metric structure
#[derive(Debug, Clone)]
struct SystemMetric {
    cpu_usage: f64,
    memory_usage: f64,
    disk_usage: f64,
    timestamp: u64,
}

/// Web request structure
#[derive(Debug, Clone)]
struct WebRequest {
    method: String,
    endpoint: String,
    status_code: u16,
}

/// Generates realistic system metrics
fn generate_system_metrics() -> SystemMetric {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    SystemMetric {
        cpu_usage: 45.2,
        memory_usage: 67.8, 
        disk_usage: 82.3,
        timestamp,
    }
}

/// Processes web request data
fn process_system_metric(data: Vec<i32>) -> Result<i32, QueueError> {
    // Simulate processing: calculate overall system health score
    if data.is_empty() {
        return Ok(50);
    }
    let avg = data.iter().sum::<i32>() / data.len() as i32;
    Ok(avg)
}

/// Configuration demonstration function
fn config_process_fn(data: Vec<i32>) -> Result<i32, QueueError> {
    // Simple processing: just return the sum
    Ok(data.iter().sum::<i32>())
}

/// Demonstrates basic queue operations
async fn basic_operations_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Basic Queue Operations Demo");
    println!("{}", "=".repeat(35));

    // Create queue configuration
    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        1000, // 1 second interval
        2,
        "system_metrics.log".to_string(),
        "system_metrics_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(config_process_fn),
        Model::Linear,
        queue_type,
    );

    let mut system_queue = Queue::new(config);
    
    // Demonstrate basic operations
    println!("🔄 Initial queue stats: size={}, capacity={}, interval={}ms", 
             system_queue.get_stats().size, system_queue.get_stats().capacity, system_queue.get_stats().interval_ms);
    
    // Publish some synthetic data manually
    let sample_data = 75; // Health score from 0-100
    system_queue.publish(sample_data).await?;
    println!("✅ Published system health score: {}", sample_data);
    
    // Get latest data  
    if let Some(latest) = system_queue.get_latest().await {
        println!("📊 Latest system health score: {}", latest.1);
    }
    
    // Show updated stats
    let stats = system_queue.get_stats();
    println!("📈 Queue stats after operations: size={}, interval={}ms", 
             stats.size, stats.interval_ms);

    Ok(())
}

/// Demonstrates AIMD functionality (with constant interval)  
async fn aimd_demonstration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧠 AIMD Adaptive Queue Demo");
    println!("{}", "=".repeat(30));

    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        500, // 500ms base interval
        3,
        "aimd_demo.log".to_string(),
        "aimd_demo_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_system_metric),
        Model::Linear,
        queue_type,
    );

    let mut aimd_queue = Queue::new(config.clone());
    let server_handle = Queue::new(config).start_server()?;
    
    println!("✅ AIMD queue started with constant interval");

    // Show AIMD stats - now constant interval
    if let Some(aimd_stats) = aimd_queue.get_aimd_stats() {
        println!("🎯 AIMD current interval: {}ms (constant mode)", aimd_stats.current_interval);
    }

    // Publish some data for AIMD processing
    for session in 0..5 {
        println!("🔄 AIMD Session {}...", session);
        let data = vec![40 + session * 10, 45 + session * 12, 50 + session * 8];
        aimd_queue.publish(50 + session).await?;
        
        sleep(Duration::from_secs(1)).await;
    }

    server_handle.shutdown().await?;
    println!("🛑 AIMD demonstration completed");
    
    Ok(())
}

/// Server demonstration
async fn server_demonstration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔧 Server Configuration Showcase");
    println!("{}", "=".repeat(35));

    // Demonstrate different server configurations
    let configs = vec![
        ("Fast Analytics", 100, 2),
        ("Medium Analytics", 500, 3), 
        ("Slow Analytics", 1000, 5),
    ];

    for (name, interval, factor) in configs {
        println!("\n⚙️  Testing {} configuration...", name);
        
        let queue_type = QueueType::new(
            QueueValue::NodeCapacity,
            interval,
            factor,
            format!("{}.log", name.to_lowercase().replace(" ", "_")),
            format!("{}_var", name.to_lowercase().replace(" ", "_")),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(process_system_metric),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config.clone());
        let server = Queue::new(config).start_server()?;
        
        // Test basic operations
        for i in 0..3 {
            queue.publish(i).await?;
        }
        
        let stats = queue.get_stats();
        println!("✅ {}: processed {} items in {}ms intervals", name, stats.size, stats.interval_ms);
        
        server.shutdown().await?;
        sleep(Duration::from_millis(500)).await;
    }

    println!("🎯 Server configuration showcase completed");
    Ok(())
}

/// Complete comprehensive demonstration
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Autoqueues Comprehensive Example");
    println!("{}", "=".repeat(40));
    println!("📊 Full-featured demonstration of autoqueues library");
    println!("🧪 Testing all major features:");
    println!("  • Queue creation and configuration");
    println!("  • Data publishing and retrieval");
    println!("  • AIMD adaptive queue management (constant interval)");
    println!("  • Server functionality");
    println!("  • Statistics and monitoring");
    println!("{}", "=".repeat(40));

    // Run all demonstrations
    basic_operations_demo().await?;
    sleep(Duration::from_secs(1)).await;
    
    aimd_demonstration().await?;
    sleep(Duration::from_secs(1)).await;
    
    server_demonstration().await?;
    sleep(Duration::from_secs(1)).await;

    println!("\n🎉 Comprehensive example completed successfully!");
    println!("\n💡 Key takeaways:");
    println!("  • Created queues with different configurations");
    println!("  • Demonstrated data flow and processing");
    println!("  • Used current AIMD with constant interval behavior");
    println!("  • Showcased server management capabilities");
    println!("  • Demonstrated real-time statistics access");

    Ok(())
}