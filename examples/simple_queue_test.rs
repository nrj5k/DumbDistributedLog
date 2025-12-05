//! Simple Queue Test
//!
//! Basic queue functionality test without server complications.

use autoqueues::{Queue, queue::SimpleQueue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Simple Queue Test");
    println!("=====================\n");

    // Test basic queue operations
    let mut queue: SimpleQueue<i32> = SimpleQueue::new();

    // Test publishing
    println!("📋 Publishing data...");
    for i in 1..=5 {
        queue.publish(i)?;
        println!("   Published: {}", i);
    }

    // Test retrieval
    println!("\n📋 Retrieving data...");
    if let Some((timestamp, latest)) = queue.get_latest() {
        println!("   Latest: {} at timestamp {}", latest, timestamp);
        assert_eq!(latest, 5);
    }

    let recent = queue.get_latest_n(3);
    println!("   Recent 3: {:?}", recent);
    assert_eq!(recent, vec![5, 4, 3]);

    println!("   ✅ Basic queue operations work correctly");
    Ok(())
}
