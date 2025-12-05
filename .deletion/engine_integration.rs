//! Simple Engine Integration Example
//!
//! Demonstrates how to use the Engine trait with AutoQueues in a clean, modular way

use autoqueues::engine::Engine;
use std::error::Error;

/// System metric input data
#[derive(Debug, Clone)]
struct SystemMetrics {
    cpu_usage: f32,
    memory_usage: f32,
    disk_usage: f32,
}

/// Health score (0-100, higher is better)
type HealthScore = u8;

/// Simple health calculation engine
struct HealthEngine;

impl Engine<SystemMetrics, HealthScore> for HealthEngine {
    fn execute(&self, metrics: SystemMetrics) -> Result<HealthScore, Box<dyn Error + Send + Sync>> {
        // Simple weighted health formula
        let cpu_score = 100.0 - metrics.cpu_usage;
        let mem_score = 100.0 - metrics.memory_usage;
        let disk_score = 100.0 - metrics.disk_usage;

        let overall_health = (cpu_score + mem_score + disk_score) / 3.0;
        Ok(overall_health.clamp(0.0, 100.0) as HealthScore)
    }

    fn name(&self) -> &str {
        "HealthEngine"
    }
}

/// Anomaly detection engine
struct AnomalyEngine {
    baseline: f32,
}

impl Engine<SystemMetrics, f32> for AnomalyEngine {
    fn execute(&self, metrics: SystemMetrics) -> Result<f32, Box<dyn Error + Send + Sync>> {
        // Calculate deviation from baseline
        let current_usage = (metrics.cpu_usage + metrics.memory_usage + metrics.disk_usage) / 3.0;
        Ok(current_usage - self.baseline)
    }

    fn name(&self) -> &str {
        "AnomalyEngine"
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("AutoQueues + Engine Trait Integration Demo");
    println!("===========================================");

    // Create engines
    let health_engine = HealthEngine;
    let anomaly_engine = AnomalyEngine { baseline: 45.0 };

    // Sample metrics
    let metrics = SystemMetrics {
        cpu_usage: 35.0,
        memory_usage: 42.0,
        disk_usage: 38.0,
    };

    // Calculate health score
    let health_score = health_engine.execute(metrics.clone())?;
    println!(
        "{}: Health Score = {}/100",
        health_engine.name(),
        health_score
    );

    // Calculate anomaly
    let anomaly = anomaly_engine.execute(metrics)?;
    println!("{}: Deviation = {:.1}%", anomaly_engine.name(), anomaly);

    println!("\nKey Points:");
    println!("- Ultra-minimal Engine trait with just execute() and name()");
    println!("- Type-safe Input/Output parameters");
    println!("- Perfect for AutoQueues health expression evaluation");
    println!("- Clean separation of algorithm from data");

    Ok(())
}
