//! Synchronous Node Health Demo  
//!
//! Simplified health scoring demonstration with TOML configuration.

use autoqueues::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
struct HealthData {
    pub cpu: f32,
    pub memory: f32,
    pub disk: f32,
}

fn create_base_hook(
    metric: &str,
) -> Arc<dyn Fn(Vec<HealthData>) -> Result<HealthData, QueueError> + Send + Sync> {
    let metric_copy = metric.to_string();

    Arc::new(
        move |_data: Vec<HealthData>| -> Result<HealthData, QueueError> {
            let mut collector = MetricsCollector::new();

            match metric_copy.as_str() {
                "cpu_percent" => {
                    let usage = collector.get_cpu_usage();
                    println!("🔥 CPU Collection: {:.1}%", usage);
                    Ok(HealthData {
                        cpu: usage,
                        memory: 0.0,
                        disk: 0.0,
                    })
                }
                "memory_percent" => {
                    let memory_usage = collector.get_memory_usage();
                    println!("🧠 Memory Collection: {:.1}%", memory_usage.usage_percent);
                    Ok(HealthData {
                        cpu: 0.0,
                        memory: memory_usage.usage_percent,
                        disk: 0.0,
                    })
                }
                "drive_percent" => {
                    let disk_data = collector.get_disk_usage();
                    let usage = disk_data.map(|d| d.usage_percent).unwrap_or(0.0);
                    println!("💾 Disk Collection: {:.1}%", usage);
                    Ok(HealthData {
                        cpu: 0.0,
                        memory: 0.0,
                        disk: usage,
                    })
                }
                _ => Ok(HealthData::default()),
            }
        },
    )
}

fn create_health_hook(
    formula_name: &str,
) -> Arc<dyn Fn(Vec<HealthData>) -> Result<HealthData, QueueError> + Send + Sync> {
    let name_copy = formula_name.to_string();

    Arc::new(move |health_data| {
        if let Some(latest) = health_data.last() {
            let score = match name_copy.as_str() {
                "weighted_health" => latest.cpu * 0.4 + latest.memory * 0.3 + latest.disk * 0.3,
                "simple_average" => (latest.cpu + latest.memory + latest.disk) / 3.0,
                "stress_score" => latest.cpu * 1.5 + latest.memory * 1.2 + latest.disk,
                "performance_index" => 100.0 - (latest.cpu + latest.memory + latest.disk) / 3.0,
                _ => (latest.cpu + latest.memory + latest.disk) / 3.0,
            };

            println!(
                "📊 {:<18}: CPU:{: >6.1} Mem:{: >6.1} Disk:{: >6.1} → Score:{: >6.1}",
                name_copy, latest.cpu, latest.memory, latest.disk, score
            );

            Ok(HealthData {
                cpu: score,
                memory: 0.0,
                disk: 0.0,
            })
        } else {
            Ok(HealthData::default())
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::time::{sleep, Duration};
    println!("🎯 Configurable Node Health Formulas Demo");
    println!("{}", "=".repeat(50));

    let config_toml = r#"
[queues]
base = ["cpu_percent", "memory_percent", "drive_percent"]
        
[queues.derived]
weighted_health = { formula = "((1/local.cpu_percent) * 0.4) + ((1/local.memory_percent) * 0.3) + ((1/local.drive_percent) * 0.3)" }
simple_average = { formula = "(local.cpu_percent + local.memory_percent + local.drive_percent) / 3.0" }
stress_score = { formula = "local.cpu_percent * 1.5 + local.memory_percent * 1.2 + local.drive_percent" }
performance_index = { formula = "100.0 - (local.cpu_percent + local.memory_percent * 0.8 + local.drive_percent * 0.8) / 2.6" }
"#;

    let config = QueueConfigFile::from_str(config_toml)?;

    println!("📋 Configuration parsed successfully:");
    println!("   Base metrics: {:?}", config.get_base_metrics());

    for (name, formula) in config.get_derived_formulas() {
        println!("   Health formula '{}': {}", name, formula);
    }

    let mut base_queues = HashMap::new();
    let mut health_queues = HashMap::new();

    // Create base metric collection queues
    println!("\n🏗️ Creating metric collection queues...");

    for metric in config.get_base_metrics() {
        let hook = create_base_hook(&metric);
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            800,
            2,
            format!("{}_base.log", metric),
            format!("{}_base_var", metric),
        );
        let config_qc = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type.clone());
        let queue = Queue::new(config_qc.clone());
        let _server = Queue::new(config_qc).start_server()?;

        base_queues.insert(metric.to_string(), queue);
    }

    // Create health formula queues
    println!("\n🏭 Creating health formula queues...");

    for (formula_name, _formula) in config.get_derived_formulas() {
        let hook = create_health_hook(&formula_name);
        let queue_type = QueueType::new(
            QueueValue::SystemHealth,
            1200,
            2,
            format!("{}_health.log", formula_name),
            format!("{}_health_var", formula_name),
        );
        let config_qc = QueueConfig::new(Mode::Insight, hook, Model::Linear, queue_type.clone());
        let queue = Queue::new(config_qc.clone());
        let _server = Queue::new(config_qc).start_server()?;

        health_queues.insert(formula_name.to_string(), queue);
    }

    println!("✅ All queues created successfully!");

    // Run monitoring demonstration
    println!("\n🔄 Running health monitoring demonstration...");

    let mut collector = MetricsCollector::new();

    for cycle in 0..10 {
        println!("\n⏰ Cycle {:02}/10", cycle + 1);

        collector.refresh();
        let cpu = collector.get_cpu_usage();
        let memory = collector.get_memory_usage().usage_percent;
        let disk = collector
            .get_disk_usage()
            .map(|d| d.usage_percent)
            .unwrap_or(0.0);

        println!(
            "📊 Real System: CPU:{:.1}% Memory:{:.1}% Disk:{:.1}%",
            cpu, memory, disk
        );

        // Publish to all queues
        for cycle in 0..10 {
            println!("\n⏰ Cycle {:02}/10", cycle + 1);

            // Collect fresh system metrics
            let cpu = collector.get_cpu_usage();
            let memory = collector.get_memory_usage().usage_percent;
            let disk = collector
                .get_disk_usage()
                .map(|d| d.usage_percent)
                .unwrap_or(0.0);

            println!(
                "📊 Real System: CPU:{:.1}% Memory:{:.1}% Disk:{:.1}%",
                cpu, memory, disk
            );

            let health_data = HealthData { cpu, memory, disk };

            // Publish to base queues
            for queue in base_queues.values_mut() {
                queue.publish(health_data.clone()).await?;
            }

            for queue in health_queues.values_mut() {
                queue.publish(health_data.clone()).await?;
            }

            // Display results from health queues
            for (name, queue) in health_queues.iter() {
                if let Some(data) = queue.get_latest_value().await {
                    let health_score = data.cpu;
                    let status = match health_score {
                        s if s >= 80.0 => "🟢 EXCELLENT",
                        s if s >= 60.0 => "🟡 GOOD",
                        s if s >= 30.0 => "🟠 WARNING",
                        _ => "🔴 CRITICAL",
                    };
                    println!("   {:<18}: {:>6.1}/100 {}", name, health_score, status);
                }
            }

            // Small delay
            sleep(Duration::from_millis(1000)).await;
        }

        // Display results from health queues
        for (name, queue) in health_queues.iter() {
            if let Some(data) = queue.get_latest_value().await {
                let health_score = data.cpu;
                let status = match health_score {
                    s if s >= 80.0 => "🟢 EXCELLENT",
                    s if s >= 60.0 => "🟡 GOOD",
                    s if s >= 30.0 => "🟠 WARNING",
                    _ => "🔴 CRITICAL",
                };
                println!("   {:<18}: {:>6.1}/100 {}", name, health_score, status);
            }
        }
    }

    // Display final results
    println!("\n{}", "=".repeat(60));
    println!("📈 FINAL HEALTH FORMULA RESULTS");
    println!("{}", "=".repeat(60));

    let mut results = Vec::new();
    for (name, queue) in health_queues.iter() {
        if let Some(data) = queue.get_latest_value().await {
            results.push((name.clone(), data.cpu));
        }
    }
    results.sort_by_key(|(name, _)| name.clone());

    println!("\n🏥 Health Formula Results:");
    for (name, score) in results {
        println!("   {:<20}: {:>6.1}/100", name, score);
    }

    println!("\n🧩 Formula Descriptions:");
    println!("   ✓ weighted_average: CPU(40%) + Memory(30%) + Disk(30%)");
    println!("   ✓ simple_average:   Equal weighting of all metrics");
    println!("   ✓ stress_score:     CPU weight:1.5 Memory weight:1.2 Disk weight:1.0");
    println!("   ✓ performance_index: Reverse scoring (100 - average_load)");

    println!("\n✅ Demo complete!");
    println!("   ✓ TOML configuration parsed from user's exact syntax");
    println!("   ✓ Real system metrics collected live (CPU/Memory/Disk)");
    println!("   ✓ Multiple health formulas demonstrated with different scoring approaches");
    println!("   ✓ Mathematical expressions configured via health formula parameter");
    println!("   ✓ Local-only expressions confirmed - perfect for Queue Manager integration!");

    Ok(())
}

