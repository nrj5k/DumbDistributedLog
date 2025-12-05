//! Example: Queue Engine Integration
//!
//! This example demonstrates how Queue<T> implements the Engine trait
//! and can be used within processing pipelines.

use autoqueues::core::Queue;
use autoqueues::engine::Engine;
use autoqueues::enums::{Mode, Model, QueueValue};
use autoqueues::types::{QueueConfig, QueueType};
use std::sync::Arc;

/// Simple queue processor function
fn simple_processor(data: Vec<i32>) -> Result<i32, autoqueues::enums::QueueError> {
    if data.is_empty() {
        return Ok(0);
    }
    let avg = data.iter().sum::<i32>() / data.len() as i32;
    println!("📊 Processor: avg of {:?} = {}", data, avg);
    Ok(avg)
}

#[tokio::main]
async fn main() {
    println!("🚀 Queue Engine Integration Demo\n");

    // Create Queue A
    let queue_type_a = QueueType::new(
        QueueValue::NodeLoad,
        1000,
        2,
        "queue_a.log".to_string(),
        "var_a".to_string(),
    );
    let config_a = QueueConfig::new(
        Mode::Sensor,
        Arc::new(simple_processor),
        Model::Linear,
        queue_type_a,
    );
    let mut queue_a = Queue::new(config_a);

    // Create Queue B
    let queue_type_b = QueueType::new(
        QueueValue::NodeCapacity,
        800,
        1,
        "queue_b.log".to_string(),
        "var_b".to_string(),
    );
    let config_b = QueueConfig::new(
        Mode::Sensor,
        Arc::new(simple_processor),
        Model::Linear,
        queue_type_b,
    );
    let mut queue_b = Queue::new(config_b);

    println!("✅ Created Queues with Engine integration:");
    println!("   📊 Queue A: {} (Engine trait)", queue_a.name());
    println!("   🔍 Queue B: {} (Engine trait)", queue_b.name());

    // Demonstrate Queue as Engine
    println!("\n🔄 Queue as Engine Processing:");

    // Test data
    let test_data = vec![10, 20, 30, 40, 50];
    println!("   Input data: {:?}", test_data);

    // Use Queue A as Engine (Engine trait)
    match queue_a.execute(test_data) {
        Ok(result) => {
            println!("   ✅ Queue A Engine result: {:?}", result);
            println!("   ✅ Engine trait working!");
        }
        Err(e) => {
            println!("   ❌ Engine error: {:?}", e);
        }
    }

    // Use Queue B as Engine
    let second_data = vec![1, 2, 3, 4, 5, 6];
    match queue_b.execute(second_data) {
        Ok(result) => {
            println!("   ✅ Queue B Engine result: {:?}", result);
        }
        Err(e) => {
            println!("   ❌ Engine error: {:?}", e);
        }
    }

    // Show Engine composability
    println!("\n🔗 Engine Composability:");
    println!("   • Queue<T> implements Engine<Vec<T>, Vec<T>>");
    println!("   • Compatible with Engine trait ecosystem");
    println!("   • Enables queue-to-queue pipelines (future)");
    println!("   • Type-safe processing contracts");

    // Demonstrate queue operations
    println!("\n📊 Regular Queue Operations:");

    // Add data to queue
    if let Err(e) = queue_a.publish(42).await {
        println!("   ❌ Publish error: {:?}", e);
    } else {
        println!("   ✅ Published value: 42");
    }

    // Get latest data
    match queue_a.get_latest().await {
        Some((timestamp, value)) => {
            println!("   📈 Latest data: {} @ {}", value, timestamp);
        }
        None => {
            println!("   📭 No data available");
        }
    }

    println!("\n✨ Queue Engine Benefits:");
    println!("   • Unified Engine+Queue interface");
    println!("   • Composable processing pipelines");
    println!("   • Type-safe data transformation");
    println!("   • Foundation for distributed queues");
    println!("   • Zero-overhead abstractions");
}
