//! # Helper Methods Working Example
//!
//! Simple demonstration of new helper methods working

use autoqueues::*;
use std::sync::Arc;

/// Current processor - must handle Vec<T> (demonstrates the problem)
fn current_processor(data: Vec<i32>) -> Result<i32, QueueError> {
    if data.is_empty() {
        return Ok(50);
    }
    let current = data.last().unwrap();
    Ok(current * 2 + 10)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Helper Methods Demo - Real Working Example");

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(current_processor),
        Model::Linear,
        QueueType::new(
            QueueValue::NodeLoad,
            200,
            2,
            "helper_demo.log".to_string(),
            "helper_demo_var".to_string(),
        ),
    );

    let mut queue = Queue::new(config);

    // Add some test data
    queue.publish(42).await?;
    queue.publish(55).await?;
    queue.publish(67).await?;

    // Demonstrate new helper methods working
    match queue.get_latest_value().await {
        Some(value) => println!("✅ get_latest_value() → {}", value),
        None => println!("❌ No data"),
    }

    let recent = queue.get_latest_n_values(3).await;
    println!("📚 get_latest_n_values(3) → {:?}", recent);

    Ok(())
}