//! DDL Basic Example

use ddl::{DdlDistributed, DDL, DdlConfig};

#[tokio::main]
async fn main() {
    println!("DDL Example\n");

    // Create a basic config
    let config = DdlConfig::default();

    // Create DDL instance (standalone mode for testing/single-node)
    let ddl = DdlDistributed::new_standalone(config);
    println!("DDL configured");

    // Push some data to a topic
    let data = b"Hello, DDL!".to_vec();
    match ddl.push("greeting", data).await {
        Ok(id) => println!("Pushed entry with ID: {}", id),
        Err(e) => println!("Error pushing: {}", e),
    }

    // Subscribe to the topic
    match ddl.subscribe("greeting").await {
        Ok(stream) => {
            // Try to receive data
            if let Some(entry) = stream.try_next() {
                println!("Received entry: ID={}, Topic={}, Payload={:?}", 
                         entry.id, entry.topic, String::from_utf8_lossy(&entry.payload));
                
                // Acknowledge the entry
                if let Err(e) = ddl.ack("greeting", entry.id).await {
                    println!("Error acknowledging: {}", e);
                } else {
                    println!("Acknowledged entry");
                }
            } else {
                println!("No data received");
            }
        },
        Err(e) => println!("Error subscribing: {}", e),
    }

    println!("Done");
}