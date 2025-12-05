//! Real System Metrics Monitoring Example
//!
//! This example demonstrates real-time system monitoring using actual CPU, memory,
//! and disk usage data from the system instead of simulated data. It shows a
//! multi-queue architecture for comprehensive system health monitoring.

use autoqueues::*;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// System health metrics data structure
#[derive(Debug, Clone)]
struct SystemHealth {
    cpu_usage: f32,
    memory_usage_percent: f32,
    disk_usage_percent: f32,
    health_score: u8,
}

/// CPU utilization processing queue
fn process_cpu_metrics(data: Vec<SystemHealth>) -> Result<SystemHealth, QueueError> {
    if let Some(metrics) = data.last() {
        println!("🔥 Queue A - CPU Usage: {:.1}%", metrics.cpu_usage);
        Ok(metrics.clone())
    } else {
        // No historical data - return default healthy state
        Ok(SystemHealth {
            cpu_usage: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            health_score: 85,
        })
    }
}

/// Memory utilization processing queue
fn process_memory_metrics(data: Vec<SystemHealth>) -> Result<SystemHealth, QueueError> {
    if let Some(metrics) = data.last() {
        println!(
            "🧠 Queue B - Memory: {:.1}% used",
            metrics.memory_usage_percent
        );
        Ok(metrics.clone())
    } else {
        Ok(SystemHealth {
            cpu_usage: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            health_score: 85,
        })
    }
}

/// Disk utilization processing queue
fn process_disk_metrics(data: Vec<SystemHealth>) -> Result<SystemHealth, QueueError> {
    if let Some(metrics) = data.last() {
        println!("💾 Queue C - Disk: {:.1}% used", metrics.disk_usage_percent);
        Ok(metrics.clone())
    } else {
        Ok(SystemHealth {
            cpu_usage: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            health_score: 85,
        })
    }
}

/// System health aggregator and alert generator
fn aggregate_system_health(data: Vec<SystemHealth>) -> Result<SystemHealth, QueueError> {
    if data.is_empty() {
        return Ok(SystemHealth {
            cpu_usage: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            health_score: 85,
        });
    }

    // Calculate rolling averages from recent data
    let recent_count = std::cmp::min(data.len(), 10);
    let recent_data = &data[data.len() - recent_count..];

    let avg_cpu: f32 = recent_data.iter().map(|m| m.cpu_usage).sum::<f32>() / recent_count as f32;
    let avg_memory: f32 = recent_data
        .iter()
        .map(|m| m.memory_usage_percent)
        .sum::<f32>()
        / recent_count as f32;
    let avg_disk: f32 = recent_data
        .iter()
        .map(|m| m.disk_usage_percent)
        .sum::<f32>()
        / recent_count as f32;

    // Generate health score based on resource utilization
    let health_score = if avg_cpu < 60.0 && avg_memory < 70.0 && avg_disk < 80.0 {
        90 // Excellent health
    } else if avg_cpu < 80.0 && avg_memory < 85.0 && avg_disk < 90.0 {
        70 // Good health
    } else if avg_cpu < 95.0 || avg_memory < 95.0 || avg_disk < 95.0 {
        50 // Warning
    } else {
        30 // Critical
    };

    println!(
        "🏥 Aggregator - System Health: {}/100 (CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%)",
        health_score, avg_cpu, avg_memory, avg_disk
    );

    Ok(SystemHealth {
        cpu_usage: avg_cpu,
        memory_usage_percent: avg_memory,
        disk_usage_percent: avg_disk,
        health_score,
    })
}

/// Collect real system metrics using our new metrics module
async fn collect_system_metrics() -> SystemHealth {
    let mut collector = MetricsCollector::new();

    // Collect actual system data
    let cpu_usage = collector.get_cpu_usage();
    let memory = collector.get_memory_usage();
    let disk = collector.get_disk_usage();
    let health_score = collector.get_health_score();

    let disk_usage = disk.map(|d| d.usage_percent).unwrap_or(0.0);

    SystemHealth {
        cpu_usage,
        memory_usage_percent: memory.usage_percent,
        disk_usage_percent: disk_usage,
        health_score,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  Real System Metrics Monitoring with Multi-Queue Architecture");
    println!("{}", "=".repeat(70));

    println!("\n📋 Monitoring Architecture:");
    println!("   Queue A: CPU utilization monitoring (1000ms intervals)");
    println!("   Queue B: Memory usage monitoring (1000ms intervals)");
    println!("   Queue C: Disk usage monitoring (1500ms intervals)");
    println!("   Aggregator: System health analysis (2000ms intervals)");

    // Queue A: CPU monitoring (1 second intervals)
    let queue_a_type = QueueType::new(
        QueueValue::NodeLoad,
        1000, // 1000ms interval
        2,    // 2x increase factor
        "cpu_metrics.log".to_string(),
        "cpu_metrics_var".to_string(),
    );

    let config_a = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_cpu_metrics),
        Model::Linear,
        queue_a_type,
    );

    // Queue B: Memory monitoring (1 second intervals)
    let queue_b_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000, // 1000ms interval
        2,    // 2x increase factor
        "memory_metrics.log".to_string(),
        "memory_metrics_var".to_string(),
    );

    let config_b = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_memory_metrics),
        Model::Linear,
        queue_b_type,
    );

    // Queue C: Disk monitoring (1.5 second intervals)
    let queue_c_type = QueueType::new(
        QueueValue::ClusterCapacity,
        1500, // 1500ms interval
        2,    // 2x increase factor
        "disk_metrics.log".to_string(),
        "disk_metrics_var".to_string(),
    );

    let config_c = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_disk_metrics),
        Model::Linear,
        queue_c_type,
    );

    // Aggregator: System health analysis (2 second intervals)
    let aggregator_type = QueueType::new(
        QueueValue::SystemHealth,
        2000, // 2000ms interval
        3,    // 3x increase factor
        "system_health_agg.log".to_string(),
        "system_health_var".to_string(),
    );

    let config_aggregator = QueueConfig::new(
        Mode::Insight,
        Arc::new(aggregate_system_health),
        Model::Linear,
        aggregator_type,
    );

    // Create queues
    let mut queue_a = Queue::new(config_a.clone());
    let mut queue_b = Queue::new(config_b.clone());
    let mut queue_c = Queue::new(config_c.clone());
    let aggregator = Queue::new(config_aggregator.clone());

    println!("\n🚀 Starting real-time monitoring servers...");

    // Start autonomous servers
    let _server_a = Queue::new(config_a).start_server()?;
    let _server_b = Queue::new(config_b).start_server()?;
    let _server_c = Queue::new(config_c).start_server()?;
    let _server_aggregator = Queue::new(config_aggregator).start_server()?;

    println!("   ✅ All monitoring servers started and collecting real data!");
    println!("\n🔄 System Monitoring Active (30-second demo)...\n");

    // Collect and process real metrics for 30 seconds
    let mut cycle = 0;
    for _ in 0..30 {
        cycle += 1;

        // Collect actual system metrics
        let metrics = collect_system_metrics().await;
        println!("⏰ Cycle {} - Real System Status:", cycle);
        println!("   🔥 CPU: {:.1}% utilization", metrics.cpu_usage);
        println!("   🧠 Memory: {:.1}% used", metrics.memory_usage_percent);
        println!("   💾 Disk: {:.1}% used", metrics.disk_usage_percent);

        // Send data to respective queues
        let _ = queue_a.publish(metrics.clone()).await;
        let _ = queue_b.publish(metrics.clone()).await;
        let _ = queue_c.publish(metrics).await;

        // Show processed data from queues
        match queue_a.get_latest().await {
            Some((_, processed_metrics)) => println!(
                "   ▪️  CPU Queue: {:.1}% processed",
                processed_metrics.cpu_usage
            ),
            None => println!("   ▪️  CPU Queue: Initializing..."),
        }

        match queue_b.get_latest().await {
            Some((_, processed_metrics)) => println!(
                "   ▪️  Memory Queue: {:.1}% processed",
                processed_metrics.memory_usage_percent
            ),
            None => println!("   ▪️  Memory Queue: Initializing..."),
        }

        match queue_c.get_latest().await {
            Some((_, processed_metrics)) => println!(
                "   ▪️  Disk Queue: {:.1}% processed",
                processed_metrics.disk_usage_percent
            ),
            None => println!("   ▪️  Disk Queue: Initializing..."),
        }

        // Show aggregated health score
        match aggregator.get_latest().await {
            Some((_, health_metrics)) => {
                let status = match health_metrics.health_score {
                    80..=100 => "🟢 EXCELLENT",
                    60..=79 => "🟡 GOOD",
                    40..=59 => "🟠 WARNING",
                    _ => "🔴 CRITICAL",
                };
                println!(
                    "   🏆 System Health: {} ({}/100)",
                    status, health_metrics.health_score
                );
                println!(
                    "      📊 Health Profile - CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%",
                    health_metrics.cpu_usage,
                    health_metrics.memory_usage_percent,
                    health_metrics.disk_usage_percent
                );
            }
            None => {
                println!("   🏆 System Health: Building health profile...");
                // Show what data is currently being collected for debugging
                let mut collector = MetricsCollector::new();
                let current_score = collector.get_health_score();
                println!(
                    "      ⏱️  Current system score would be: {}/100",
                    current_score
                );
            }
        }

        println!();
        sleep(Duration::from_millis(1000)).await; // 1-second cycle
    }

    // Final system statistics
    println!("\n📊 Final Monitoring Statistics:");
    println!("Queue A (CPU):        {:?}", queue_a.get_stats());
    println!("Queue B (Memory):     {:?}", queue_b.get_stats());
    println!("Queue C (Disk):       {:?}", queue_c.get_stats());
    println!("Aggregator (Health):  {:?}", aggregator.get_stats());

    // Show current real-time health assessment
    let final_metrics = collect_system_metrics().await;
    println!("\n🔋 Current Real System Status:");
    println!("   CPU: {:.1}% utilization", final_metrics.cpu_usage);
    println!("   Memory: {:.1}% used", final_metrics.memory_usage_percent);
    println!("   Disk: {:.1}% used", final_metrics.disk_usage_percent);

    match aggregator.get_latest().await {
        Some((_, health)) => {
            let status = if health.health_score >= 70 {
                "STABLE"
            } else {
                "NEEDS ATTENTION"
            };
            println!(
                "\n🏥 Overall System Health: {} ({}/100)",
                status, health.health_score
            );
            println!("    🧩 Detailed Profile:");
            println!("       ✓ CPU Component: {:.1}% average", health.cpu_usage);
            println!(
                "       ✓ Memory Component: {:.1}% average",
                health.memory_usage_percent
            );
            println!(
                "       ✓ Disk Component: {:.1}% average",
                health.disk_usage_percent
            );

            if final_metrics.cpu_usage > 80.0 {
                println!(
                    "⚠️  WARNING: High CPU usage detected - {:.1}%",
                    final_metrics.cpu_usage
                );
            }
            if final_metrics.memory_usage_percent > 85.0 {
                println!(
                    "⚠️  WARNING: High memory usage detected - {:.1}%",
                    final_metrics.memory_usage_percent
                );
            }
            if final_metrics.disk_usage_percent > 90.0 {
                println!(
                    "⚠️  WARNING: Critical disk usage detected - {:.1}%",
                    final_metrics.disk_usage_percent
                );
            }
        }
        None => println!("\n⏳ Health assessment in progress..."),
    }

    println!("\n🎉 Real system metrics monitoring completed successfully!");
    println!("   Multi-queue architecture with real data collection working correctly.");
    println!("   This example demonstrates actual system resource monitoring");
    println!("   with autonomous queue processing and health aggregation.");

    Ok(())
}
