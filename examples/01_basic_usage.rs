//! Basic usage example for the DDL API

use ddl::{DdlDistributed, DDL, DdlConfig};

#[tokio::main]
async fn main() {
    // Create a basic config
    let config = DdlConfig::default();

    // Create DDL instance (standalone mode for testing/single-node)
    let ddl = DdlDistributed::new_standalone(config);

    // Push some data to CPU topic
    let cpu_data = b"CPU usage: 42%".to_vec();
    match ddl.push("cpu", cpu_data).await {
        Ok(id) => println!("Pushed CPU data with ID: {}", id),
        Err(e) => println!("Error pushing CPU data: {}", e),
    }

    // Push some data to memory topic
    let mem_data = b"Memory usage: 60%".to_vec();
    match ddl.push("memory", mem_data).await {
        Ok(id) => println!("Pushed memory data with ID: {}", id),
        Err(e) => println!("Error pushing memory data: {}", e),
    }

    // Subscribe to CPU topic and read data
    match ddl.subscribe("cpu").await {
        Ok(stream) => {
            if let Some(entry) = stream.try_next() {
                println!("Received CPU entry: {:?}", String::from_utf8_lossy(&entry.payload));
                // Acknowledge the entry
                if let Err(e) = ddl.ack("cpu", entry.id).await {
                    println!("Error acknowledging CPU entry: {}", e);
                } else {
                    println!("Acknowledged CPU entry");
                }
            }
        },
        Err(e) => println!("Error subscribing to CPU: {}", e),
    }

    println!("DDL initialized successfully!");
}