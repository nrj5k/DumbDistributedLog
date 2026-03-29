//! Aggregation Demo
//!
//! This example demonstrates how to use the distributed aggregation system
//! in DDL to compute global metrics across multiple nodes.

use ddl::ddl_distributed::DdlDistributed;
use ddl::traits::ddl::{DDL, DdlConfig};
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Aggregation Demo");

    // Create configuration
    let mut config = DdlConfig::default();
    config.owned_topics = vec!["cpu_node1".to_string(), "memory_node1".to_string()];
    config.gossip_enabled = true;
    config.gossip_bind_addr = "127.0.0.1:0".to_string(); // Let OS assign port

    // Create DDL instance
    let ddl = DdlDistributed::new_distributed(config).await?;

    // Push some sample data (simulate metrics)
    let cpu_data = b"CPU usage: 45.2%".to_vec();
    match ddl.push("cpu_node1", cpu_data).await {
        Ok(id) => println!("Pushed CPU data with ID: {}", id),
        Err(e) => println!("Error pushing CPU data: {}", e),
    }

    let memory_data = b"Memory usage: 65.8%".to_vec();
    match ddl.push("memory_node1", memory_data).await {
        Ok(id) => println!("Pushed memory data with ID: {}", id),
        Err(e) => println!("Error pushing memory data: {}", e),
    }

    // Subscribe and read data back
    match ddl.subscribe("cpu_node1").await {
        Ok(stream) => {
            if let Some(entry) = stream.try_next() {
                println!("Received CPU entry: {:?}", String::from_utf8_lossy(&entry.payload));
                // Acknowledge the entry
                match ddl.ack("cpu_node1", entry.id).await {
                    Ok(_) => println!("Acknowledged CPU entry"),
                    Err(e) => println!("Error acknowledging CPU entry: {}", e),
                }
            }
        },
        Err(e) => println!("Error subscribing to CPU: {}", e),
    }

    println!("Queues started. Waiting for data...");

    // Run for 2 seconds
    thread::sleep(Duration::from_secs(2));

    println!("Demo completed.");
    Ok(())
}
