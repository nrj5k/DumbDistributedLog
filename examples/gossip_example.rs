//! Example showing how to use gossip-based topic distribution with DDL

use ddl::traits::ddl::{DDL, DdlConfig};
use ddl::ddl_distributed::DdlDistributed;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a DDL config with gossip enabled
    let mut config = DdlConfig::default();
    config.node_id = 1;
    config.gossip_enabled = true;
    config.gossip_bind_addr = "127.0.0.1:7001".to_string();
    config.gossip_bootstrap = vec!["127.0.0.1:7001".to_string()]; // Self-bootstrap for demo
    config.owned_topics = vec!["metrics.cpu".to_string(), "metrics.memory".to_string()];
    config.buffer_size = 1024;
    
    println!("Creating distributed DDL with gossip...");
    
    // Create distributed DDL
    let ddl = DdlDistributed::new(config).await?;
    
    println!("Created distributed DDL");
    
    // Subscribe to CPU metrics
    println!("Subscribing to metrics.cpu...");
    let stream = ddl.subscribe("metrics.cpu").await?;
    
    // Push some data
    println!("Pushing data to metrics.cpu...");
    let entry_id = ddl.push("metrics.cpu", b"CPU usage: 42%".to_vec()).await?;
    println!("Pushed entry with ID: {}", entry_id);
    
    // Try to receive the data
    println!("Waiting for data...");
    sleep(Duration::from_millis(100)).await;
    
    // Try to get data from the stream
    if let Some(entry) = stream.try_next() {
        println!("Received entry: ID={}, Topic={}, Payload={:?}", 
                 entry.id, entry.topic, String::from_utf8_lossy(&entry.payload));
    } else {
        println!("No data received (this is expected in this simple example)");
    }
    
    // Acknowledge the entry
    ddl.ack("metrics.cpu", entry_id).await?;
    println!("Acknowledged entry");
    
    // Check position
    let position = ddl.position("metrics.cpu").await?;
    println!("Current position for metrics.cpu: {}", position);
    
    println!("Example completed successfully!");
    
    Ok(())
}