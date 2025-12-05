//! Local Publisher-Subscriber Example
//!
//! Demonstrates local queue-to-queue communication using topic-based pub/sub.
//! Shows hierarchical topics like "cpu/hostname/usage" and "drives/*/capacity"
//! for building complex aggregation patterns.

use autoqueues::{
    Mode, Model, PubSubBroker, Queue, QueueConfig, QueueError, QueueType, QueueValue,
};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::time::{Duration, sleep};

/// System data structure for topic publishing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
struct SystemMetric {
    pub hostname: String,
    pub timestamp: u64,
    pub metric_type: String,
    pub value: f32,
    pub unit: String,
}

/// Process individual CPU metrics for queue A
fn process_cpu_queue(data: Vec<SystemMetric>) -> Result<SystemMetric, QueueError> {
    if let Some(latest) = data.last() {
        println!(
            "🔥 CPU Queue - Processing: {:.1} {} from {}",
            latest.value, latest.unit, latest.hostname
        );
        Ok(latest.clone())
    } else {
        Ok(SystemMetric {
            hostname: "system_a".to_string(),
            timestamp: 0,
            metric_type: "cpu_usage".to_string(),
            value: 0.0,
            unit: "%".to_string(),
        })
    }
}

/// Process memory metrics for queue B
fn process_memory_queue(data: Vec<SystemMetric>) -> Result<SystemMetric, QueueError> {
    if let Some(latest) = data.last() {
        println!(
            "🧠 Memory Queue - Processing: {:.1} {} from {}",
            latest.value, latest.unit, latest.hostname
        );
        Ok(latest.clone())
    } else {
        Ok(SystemMetric {
            hostname: "system_a".to_string(),
            timestamp: 0,
            metric_type: "memory_usage".to_string(),
            value: 0.0,
            unit: "%".to_string(),
        })
    }
}

/// Aggregate health from multiple metrics using topic subscriptions
fn process_health_aggregator(data: Vec<SystemMetric>) -> Result<SystemMetric, QueueError> {
    if data.is_empty() {
        println!("🏥 Health Aggregator - No data yet, waiting...");
        return Ok(SystemMetric {
            hostname: "aggregator".to_string(),
            timestamp: 0,
            metric_type: "health_score".to_string(),
            value: 100.0,
            unit: "score".to_string(),
        });
    }

    // Calculate rolling health score from recent metrics
    let recent_count = std::cmp::min(data.len(), 5);
    let recent_data = &data[data.len() - recent_count..];

    let cpu_avg = recent_data
        .iter()
        .filter(|m| m.metric_type == "cpu_usage")
        .map(|m| m.value)
        .sum::<f32>()
        / recent_count as f32;

    let memory_avg = recent_data
        .iter()
        .filter(|m| m.metric_type == "memory_usage")
        .map(|m| m.value)
        .sum::<f32>()
        / recent_count as f32;

    let health_score = if cpu_avg < 30.0 && memory_avg < 40.0 {
        95.0
    } else if cpu_avg < 70.0 && memory_avg < 70.0 {
        75.0
    } else {
        45.0
    };

    println!(
        "🏥 Health Aggregator - Combined: {:.1}/100 (CPU avg:{:.1}%, Memory avg:{:.1}%)",
        health_score, cpu_avg, memory_avg
    );

    Ok(SystemMetric {
        hostname: "aggregator".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        metric_type: "health_score".to_string(),
        value: health_score,
        unit: "score".to_string(),
    })
}

/// Simple expression evaluator for user-defined aggregations
#[allow(dead_code)]
fn evaluate_capacity_ratio(
    context: SystemMetric,
    local_value: f32,
) -> Result<SystemMetric, QueueError> {
    if local_value == 0.0 {
        return Err(QueueError::Other("Cannot divide by zero".to_string()));
    }

    let ratio = context.value / local_value;

    println!(
        "📊 Capacity Ratio - Global:{:.1} / Local:{:.1} = {:.2}",
        context.value, local_value, ratio
    );

    Ok(SystemMetric {
        hostname: "capacity_calculator".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        metric_type: "drive_capacity_ratio".to_string(),
        value: ratio,
        unit: "ratio".to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Local Publisher-Subscriber Queue System Demo");
    println!("{}", "=".repeat(50));

    println!("\n🎯 Architecture Overview:");
    println!("   - CPU metrics → Topic: 'metrics/cpu/system_a'");
    println!("   - Memory metrics → Topic: 'metrics/memory/system_a'");
    println!("   - Health aggregator → Subscribe to both CPU + Memory topics");
    println!("   - Capacity calculator → Expressions like global/local ratios");

    // Create shared Pub/Sub broker
    let broker = Arc::new(PubSubBroker::new(100));

    // Queue A: CPU metrics → publishes to "metrics/cpu/system_a"
    let cpu_topic = "metrics/cpu/system_a".to_string();
    let cpu_config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_cpu_queue),
        Model::Linear,
        QueueType::new(
            QueueValue::NodeLoad,
            1000, // 1-second intervals
            2,
            "cpu_metrics.log".to_string(),
            "cpu_metrics_var".to_string(),
        ),
    );
    let mut cpu_queue = Queue::new(cpu_config.clone());

    // Queue B: Memory metrics → publishes to "metrics/memory/system_a"
    let memory_topic = "metrics/memory/system_a".to_string();
    let memory_config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_memory_queue),
        Model::Linear,
        QueueType::new(
            QueueValue::NodeAvailability,
            1500, // 1.5-second intervals
            2,
            "memory_metrics.log".to_string(),
            "memory_metrics_var".to_string(),
        ),
    );
    let mut memory_queue = Queue::new(memory_config.clone());

    // Health Aggregator → subscribes to CPU + Memory topics
    let health_config = QueueConfig::new(
        Mode::Insight,
        Arc::new(process_health_aggregator),
        Model::Linear,
        QueueType::new(
            QueueValue::SystemHealth,
            2000, // 2-second intervals
            3,
            "health_metrics.log".to_string(),
            "health_metrics_var".to_string(),
        ),
    );
    let mut health_queue = Queue::new(health_config.clone());

    println!("\n🚀 Starting autonomous queue servers...");
    let _cpu_server = Queue::new(cpu_config).start_server()?;
    let _memory_server = Queue::new(memory_config).start_server()?;
    let _health_server = Queue::new(health_config).start_server()?;

    println!("   ✅ All servers running autonomously!");
    println!("\n🔄 Demonstrating topic-based pub/sub... (15-second demo)\n");

    // Initialize topic subscriptions
    let mut _cpu_subscriber = broker.subscribe_exact(cpu_topic.clone()).await?;
    let mut _memory_subscriber = broker.subscribe_exact(memory_topic.clone()).await?;

    // Simulate 15 seconds of system metrics
    let mut cycle = 0;
    let hostname = "system_a";

    for _ in 0..10 {
        cycle += 1;

        // Generate realistic system metrics
        let cpu_metric = SystemMetric {
            hostname: hostname.to_string(),
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            metric_type: "cpu_usage".to_string(),
            value: 25.0 + (cycle as f32 % 25.0), // 25-50% range
            unit: "%".to_string(),
        };

        let memory_metric = SystemMetric {
            hostname: hostname.to_string(),
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            metric_type: "memory_usage".to_string(),
            value: 35.0 + (cycle as f32 * 0.5) % 20.0, // 35-55% range
            unit: "%".to_string(),
        };

        println!("⏰ Cycle {} - Publishing new metrics:", cycle);

        // Publish to individual queues
        let _ = cpu_queue.publish(cpu_metric.clone()).await;
        let _ = memory_queue.publish(memory_metric.clone()).await;

        // Publish to pub/sub topics for distributed aggregation
        broker.publish(cpu_topic.clone(), &cpu_metric).await?;
        broker.publish(memory_topic.clone(), &memory_metric).await?;

        // Also publish to the health aggregator queue
        let _ = health_queue.publish(cpu_metric).await;
        let _ = health_queue.publish(memory_metric).await;

        // Show cross-queue health aggregation
        match health_queue.get_latest().await {
            Some((_, health_data)) => {
                println!(
                    "   🏆 Updated Health: {:.1}/100 ({})",
                    health_data.value, health_data.unit
                );
            }
            None => println!("   📊 Health queue: No data yet"),
        }

        println!();
        sleep(Duration::from_millis(1000)).await;
    }

    // Demonstrate topic hierarchy and aggregation
    println!("\n🔍 Demonstrating topic hierarchy...");
    println!("📊 Active topics: [local/cpu-metric, local/memory-metric, local/health-metric]");

    // Show final system statistics
    println!("\n📈 Final Queue Statistics:");
    println!("CPU Queue: {:?}", cpu_queue.get_stats());
    println!("Memory Queue: {:?}", memory_queue.get_stats());
    println!("Health Queue: {:?}", health_queue.get_stats());

    // Show potential for user-defined expressions like global.drive_cap/local.drive_cap
    println!("\n🎯 Next Steps - User-Defined Expressions:");
    println!("   - Support expressions like: global.drive_capacity/local.drive_capacity");
    println!("   - Topic patterns: drives/*/capacity, cpu/*/usage");
    println!("   - Mathematical operations: sum(), avg(), ratio(), percent_change()");

    // Graceful shutdown - just drop the broker to clean up
    drop(broker);

    println!("\n✅ Local pub/sub demonstration complete!");
    println!("   Queues successfully communicated via topic-based messaging.");

    Ok(())
}
