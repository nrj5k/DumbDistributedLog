//! Health Score Demo - Standalone System Health Check
//!
//! This example demonstrates the actual health scoring system with immediate
//! visibility of the node/health score calculations.

use autoqueues::*;
use std::sync::Arc;

// System data structure for health modeling
#[derive(Debug, Clone)]
struct HealthData {
    cpu_usage: f32,
    memory_usage: f32,
    disk_usage: f32,
}

// Direct health score calculation for transparency
fn calculate_health_score(cpu: f32, memory: f32, disk: f32) -> u8 {
    let cpu_score = if cpu < 50.0 {
        25
    } else if cpu < 80.0 {
        15
    } else {
        5
    };
    let memory_score = if memory < 50.0 {
        25
    } else if memory < 80.0 {
        15
    } else {
        5
    };
    let disk_score = if disk < 70.0 {
        25
    } else if disk < 90.0 {
        15
    } else {
        5
    };
    let base_score = 25; // System uptime/health buffer

    (cpu_score + memory_score + disk_score + base_score) as u8
}

// Real-time health aggregator with transparent scoring
fn aggregate_with_transparent_scoring(data: Vec<HealthData>) -> Result<HealthData, QueueError> {
    println!("  📊 HEALTH CALCULATION DEBUG:");

    if data.is_empty() {
        println!("    ⚠️  No data available, using safe defaults");
        return Ok(HealthData {
            cpu_usage: 10.0,
            memory_usage: 30.0,
            disk_usage: 50.0,
        });
    }

    // Calculate averages from recent data points
    let recent_count = std::cmp::min(data.len(), 10);
    let recent_data = &data[data.len() - recent_count..];

    let avg_cpu = recent_data.iter().map(|m| m.cpu_usage).sum::<f32>() / recent_count as f32;
    let avg_memory = recent_data.iter().map(|m| m.memory_usage).sum::<f32>() / recent_count as f32;
    let avg_disk = recent_data.iter().map(|m| m.disk_usage).sum::<f32>() / recent_count as f32;

    println!(
        "    📋 Input Averages - CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%",
        avg_cpu, avg_memory, avg_disk
    );

    // Break down the scoring algorithm
    let cpu_score = if avg_cpu < 50.0 {
        25
    } else if avg_cpu < 80.0 {
        15
    } else {
        5
    };
    let memory_score = if avg_memory < 50.0 {
        25
    } else if avg_memory < 80.0 {
        15
    } else {
        5
    };
    let disk_score = if avg_disk < 70.0 {
        25
    } else if avg_disk < 90.0 {
        15
    } else {
        5
    };
    let base_score = 25;

    let total_score = cpu_score + memory_score + disk_score + base_score;

    println!("    🧮 SCORE BREAKDOWN:");
    println!(
        "       CPU Component: {}/25 {:.1}% usage",
        cpu_score, avg_cpu
    );
    println!(
        "       Memory Component: {}/25 {:.1}% usage",
        memory_score, avg_memory
    );
    println!(
        "       Disk Component: {}/25 {:.1}% usage",
        disk_score, avg_disk
    );
    println!("       Base Score: {}/25 system health", base_score);
    println!("    🏆 FINAL NODE SCORE: {}/100", total_score);

    // Return the averaged metrics for continuity
    Ok(HealthData {
        cpu_usage: avg_cpu,
        memory_usage: avg_memory,
        disk_usage: avg_disk,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 HEALTH SCORE DEMONSTRATION");
    println!("{}", "=".repeat(40));

    // Demo the scoring algorithm with real system data
    println!("\n🔍 REAL SYSTEM HEALTH CHECK:");

    let mut collector = MetricsCollector::new();
    let _current_health = collector.get_health_score();
    let cpu_usage = collector.get_cpu_usage();
    let memory = collector.get_memory_usage();
    let disk_usage = collector
        .get_disk_usage()
        .map(|d| d.usage_percent)
        .unwrap_or(0.0);

    println!("📊 Current System Metrics:");
    println!("   🔥 CPU Usage: {:.1}%", cpu_usage);
    println!("   🧠 Memory Usage: {:.1}%", memory.usage_percent);
    println!("   💾 Disk Usage: {:.1}%", disk_usage);

    let node_score = calculate_health_score(cpu_usage, memory.usage_percent, disk_usage);
    println!("🏆 CURRENT NODE HEALTH SCORE: {}/100", node_score);

    // Show scoring breakdown
    println!("\n🧮 DETAILED SCORING ALGORITHM:");
    let cpu_score = if cpu_usage < 50.0 {
        25
    } else if cpu_usage < 80.0 {
        15
    } else {
        5
    };
    let memory_score = if memory.usage_percent < 50.0 {
        25
    } else if memory.usage_percent < 80.0 {
        15
    } else {
        5
    };
    let disk_score = if disk_usage < 70.0 {
        25
    } else if disk_usage < 90.0 {
        15
    } else {
        5
    };

    println!(
        "   CPU Component Score:     {}/25 (for {:.1}% usage)",
        cpu_score, cpu_usage
    );
    println!(
        "   Memory Component Score:  {}/25 (for {:.1}% usage)",
        memory_score, memory.usage_percent
    );
    println!(
        "   Disk Component Score:    {}/25 (for {:.1}% usage)",
        disk_score, disk_usage
    );
    println!("   Base System Score:       25/25 (system buffer)");
    println!("   TOTAL HEALTH SCORE:      {}/100", node_score);

    // Demo the queue-based aggregation system
    println!("\n🔄 DEMONSTRATING QUEUE-BASED HEALTH AGGREGATION:");

    let queue_config = QueueConfig::new(
        Mode::Insight,
        Arc::new(aggregate_with_transparent_scoring),
        Model::Linear,
        QueueType::new(
            QueueValue::SystemHealth,
            1000,
            2,
            "health_demo.log".to_string(),
            "health_demo_var".to_string(),
        ),
    );

    let mut health_queue = Queue::new(queue_config.clone());
    let _server = Queue::new(queue_config).start_server()?;

    // Simulate health data over time
    let health_samples = vec![
        HealthData {
            cpu_usage: 20.0,
            memory_usage: 35.0,
            disk_usage: 60.0,
        },
        HealthData {
            cpu_usage: 25.0,
            memory_usage: 35.0,
            disk_usage: 60.0,
        },
        HealthData {
            cpu_usage: 30.0,
            memory_usage: 35.0,
            disk_usage: 60.0,
        },
        HealthData {
            cpu_usage: 15.0,
            memory_usage: 35.0,
            disk_usage: 60.0,
        },
        HealthData {
            cpu_usage: 10.0,
            memory_usage: 35.0,
            disk_usage: 60.0,
        },
    ];

    println!("\n📈 PROCESSING HEALTH DATA SAMPLES:");
    for (i, sample) in health_samples.iter().enumerate() {
        health_queue.publish(sample.clone()).await?;

        // Show immediate feedback
        let node_score =
            calculate_health_score(sample.cpu_usage, sample.memory_usage, sample.disk_usage);
        println!(
            "   Sample {}: CPU {:.1}%, Memory {:.1}%, Disk {:.1}% → NODE SCORE: {}/100",
            i + 1,
            sample.cpu_usage,
            sample.memory_usage,
            sample.disk_usage,
            node_score
        );

        // Show queue aggregation if available
        if let Some((_, aggregated_data)) = health_queue.get_latest().await {
            println!(
                "   📊 AGGREGATED HEALTH: CPU {:.1}%, Memory {:.1}%, Disk {:.1}%",
                aggregated_data.cpu_usage, aggregated_data.memory_usage, aggregated_data.disk_usage
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Final health assessment
    println!("\n🏁 FINAL HEALTH ASSESSMENT:");
    if let Some((_, final_health)) = health_queue.get_latest().await {
        let final_score = calculate_health_score(
            final_health.cpu_usage,
            final_health.memory_usage,
            final_health.disk_usage,
        );
        println!("   OVERALL SYSTEM NODE SCORE: {}/100", final_score);
        println!("   📋 FINAL AGGREGATED METRICS:");
        println!("      Average CPU Usage: {:.1}%", final_health.cpu_usage);
        println!(
            "      Average Memory Usage: {:.1}%",
            final_health.memory_usage
        );
        println!("      Average Disk Usage: {:.1}%", final_health.disk_usage);
    }

    println!("\n✅ Health scoring complete!");
    println!("   The system provides transparent scoring with component breakdowns.");
    println!("   Score interpretation: 80-100=EXCELLENT, 60-79=GOOD, 40-59=WARNING, <40=CRITICAL");

    Ok(())
}
