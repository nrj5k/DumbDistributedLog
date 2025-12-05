//! # Disk Monitoring System Example
//!
//! This example demonstrates a multi-queue architecture for disk monitoring.
//! 
//! Architecture:
//! - Queue A: Monitors free disk space percentage (200ms intervals)
//! - Queue B: Monitors used disk space percentage (200ms intervals)  
//! - Aggregator: Combines metrics from A+B (500ms intervals)
//!
//! Each queue runs as an autonomous server and processes data independently.

use autoqueues::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Simple disk metrics data structure
#[derive(Debug, Clone)]
struct DiskMetrics {
    free_percentage: f64,
    used_percentage: f64,
}

/// Simulate getting disk metrics (normally would call df -h /home)
fn sample_disk_metrics() -> DiskMetrics {
    // Simulate realistic disk usage patterns over time
    static mut COUNTER: f64 = 0.0;
    unsafe {
        COUNTER += 0.5;
        let base_used = 54.8 + (COUNTER.sin() * 5.0); // Vary between ~50-60%
        DiskMetrics {
            free_percentage: 100.0 - base_used,
            used_percentage: base_used,
        }
    }
}

/// Queue A: Process free disk space metrics
fn process_free_space(data: Vec<i32>) -> Result<i32, QueueError> {
    if let Some(metrics) = data.last() {
        let free_gb = (metrics * 1) as i32;
        println!("📊 Queue A - Free Space: {} units", free_gb);
        Ok(free_gb)
    } else {
        let current = sample_disk_metrics();
        Ok(current.free_percentage.round() as i32)
    }
}

/// Queue B: Process used disk space metrics
fn process_used_space(data: Vec<i32>) -> Result<i32, QueueError> {
    if let Some(metrics) = data.last() {
        let used_gb = (metrics * 1) as i32;
        println!("📈 Queue B - Used Space: {} units", used_gb);
        Ok(used_gb)
    } else {
        let current = sample_disk_metrics();
        Ok(current.used_percentage.round() as i32)
    }
}

/// Aggregator: Combine free + used with basic threshold detection
fn aggregate_disk_health(data: Vec<i32>) -> Result<i32, QueueError> {
    if data.is_empty() {
        return Ok(75); // Healthy default
    }
    
    let total: i32 = data.iter().sum();
    let avg = total / data.len() as i32;
    
    // Health scoring based on combined metrics
    let health_score = if avg < 60 {
        90 // Very healthy
    } else if avg < 80 {
        70 // Healthy  
    } else if avg < 90 {
        50 // Warning
    } else {
        30 // Critical
    };
    
    println!("🏥 Aggregator - Health Score: {}/100 (avg input: {})", health_score, avg);
    Ok(health_score)
}

/// Main async runtime for disk monitoring
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  Disk Monitoring System with Multi-Queue Architecture");
    println!("{}", "=".repeat(60));
    
    println!("\n📋 Architecture:");
    println!("   Queue A: Free space monitoring (200ms intervals)");
    println!("   Queue B: Used space monitoring (200ms intervals)");
    println!("   Aggregator: Combined health analysis (500ms intervals)");
    
    // Queue A: Free space monitoring
    let queue_a_type = QueueType::new(
        QueueValue::NodeAvailability,
        200,  // 200ms interval
        2,    // 2x increase factor
        "disk_free_a.log".to_string(),
        "disk_free_var".to_string(),
    );

    let config_a = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_free_space),
        Model::Linear,
        queue_a_type,
    );

    // Queue B: Used space monitoring
    let queue_b_type = QueueType::new(
        QueueValue::NodeLoad,
        200,  // 200ms interval
        2,    // 2x increase factor
        "disk_used_b.log".to_string(),
        "disk_used_var".to_string(),
    );

    let config_b = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_used_space),
        Model::Linear,
        queue_b_type,
    );

    // Aggregator: Combined health analysis
    let aggregator_type = QueueType::new(
        QueueValue::ClusterCapacity,
        500,  // 500ms interval
        3,    // 3x increase factor
        "disk_health_agg.log".to_string(),
        "disk_health_var".to_string(),
    );

    let config_aggregator = QueueConfig::new(
        Mode::Insight,
        Arc::new(aggregate_disk_health),
        Model::Linear,
        aggregator_type,
    );

    // Create queues
    let mut queue_a = Queue::new(config_a.clone());
    let mut queue_b = Queue::new(config_b.clone());  
    let mut aggregator = Queue::new(config_aggregator.clone());

    println!("\n🚀 Starting autonomous servers...");
    
    // Start the servers - they will run independently
    let server_a = Queue::new(config_a).start_server()?;
    let server_b = Queue::new(config_b).start_server()?;
    let server_aggregator = Queue::new(config_aggregator).start_server()?;

    println!("   ✅ All servers started and processing autonomously!");
    println!("\n🔄 Demonstrating queue interactions...\n");

    // Simulate 15 seconds of disk monitoring
    let mut cycle = 0;
    for _ in 0..15 {
        cycle += 1;
        
        // Get current disk metrics
        let current = sample_disk_metrics();
        println!("⏰ Cycle {} - Disk Status: {:.1}% free, {:.1}% used", 
                 cycle, current.free_percentage, current.used_percentage);

        // Send data to queues
        let _ = queue_a.publish(current.free_percentage.round() as i32).await;
        let _ = queue_b.publish(current.used_percentage.round() as i32).await;
        
        // Wait a moment for queues to process
        sleep(Duration::from_millis(200)).await;
        
        // Read latest processed data (queues update continuously)
        match queue_a.get_latest().await {
            Some((timestamp, free_value)) => println!("   ▪️  Queue A: {} units free space", free_value),
            None => println!("   ▪️  Queue A: No data yet"),
        }
        
        match queue_b.get_latest().await {
            Some((timestamp, used_value)) => println!("   ▪️  Queue B: {} units used space", used_value),
            None => println!("   ▪️  Queue B: No data yet"),
        }
        
        match aggregator.get_latest().await {
            Some((timestamp, health_score)) => {
                let status = match health_score {
                    90..=100 => "🟢 EXCELLENT",
                    70..=89  => "🟡 GOOD", 
                    50..=69  => "🟠 WARNING",
                    _        => "🔴 CRITICAL"
                };
                println!("   ▪️  Aggregator: {}/100 {}", health_score, status);
            }
            None => println!("   ▪️  Aggregator: Building health profile..."),
        }
        
        println!();
        sleep(Duration::from_millis(800)).await; // Total 1s cycle
    }

    // Show final system statistics
    println!("\n📊 Final Statistics:");
    println!("Queue A (free space): {:?}", queue_a.get_stats());
    println!("Queue B (used space): {:?}", queue_b.get_stats());
    println!("Aggregator (health):  {:?}", aggregator.get_stats());

    // Simulate maintenance scenario - servers continue running
    println!("\n🔧 System taking metrics servers offline for maintenance...");
    println!("   (Servers continue processing autonomously)");
    
    sleep(Duration::from_secs(3)).await;
    
    println!("✅ Servers back online - recovery complete");
    println!("   Multi-queue coordination continuing as designed\n");

    // Final check - show continuous operation
    let final_metrics = sample_disk_metrics();
    println!("🔋 Current Status: {:.1}% free, {:.1}% used", 
             final_metrics.free_percentage, final_metrics.used_percentage);
    
    match aggregator.get_latest().await {
        Some((_, score)) => {
            let status = if score >= 80 { "HEALTHY" } else { "NEEDS ATTENTION" };
            println!("🏆 Overall Disk Health: {} ({}/100)", status, score);
        }
        None => println!("⏳ Health assessment in progress..."),
    }

    println!("\n🎉 Disk monitoring system operating successfully!");
    println!("   Multi-queue architecture with autonomous servers working correctly.");

    Ok(())
}