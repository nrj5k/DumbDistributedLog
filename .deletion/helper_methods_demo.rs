//! Helper Methods Demonstration
//!
//! This example demonstrates new helper methods added to Queue<T>
//!
//! Purpose: Shows the solution to Vec<T> forced processing patterns
//! that require defensive programming, enabling clean data access.

use autoqueues::*;
use std::sync::Arc;

fn current_processor(data: Vec<i32>) -> Result<i32, autoqueues::enums::QueueError> {
    if data.is_empty() {
        return Ok(50);
    }
    let current = data.last().unwrap();
    Ok(current * 2 + 10)
}

fn new_processor(current: i32) -> Result<i32, autoqueues::enums::QueueError> {
    Ok(current * 2 + 10)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Helper Methods - Problem & Solution Demo");
    println!();
    println!("60 characters separator");
    println!();

    println!("CURRENT PROBLEM: Vec<T> Forced Pattern");
    println!("  Even simple transforms require Vec<T> handling");
    println!();

    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        200,
        2,
        "helper_demo.log".to_string(),
        "helper_demo_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(current_processor),
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config);

    queue.publish(42).await?;
    queue.publish(55).await?;
    queue.publish(67).await?;
    queue.publish(80).await?;
    queue.publish(93).await?;

    println!("Data published: [42, 55, 67, 80, 93]");

    if let Some(latest) = queue.get_latest().await {
        let (_, processed) = latest;
        println!("Latest processed: {} (via Vec<T> pattern)", processed);
    }

    println!();
    println!("NEW HELPER METHODS - Solution");

    if let Some(current_value) = queue.get_latest_value().await {
        println!("get_latest_value() -> {} (clean access)", current_value);
    }

    let recent = queue.get_latest_n_values(3).await;
    println!("get_latest_n_values(3) -> {:?}", recent);

    let stats = queue.get_stats();
    println!("Queue: {} items", stats.size);

    Ok(())
}
