//! Example demonstrating the batch push functionality

use ddl::{DdlDistributed, DDL, DdlConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create DDL with default config (standalone mode)
    let config = DdlConfig::default();
    let ddl = DdlDistributed::new_standalone(config);
    
    // Subscribe to the topic before pushing messages
    let stream = ddl.subscribe("test.batch").await?;
    
    // Prepare a batch of payloads
    let payloads = vec![
        b"First message".to_vec(),
        b"Second message".to_vec(),
        b"Third message".to_vec(),
        b"Fourth message".to_vec(),
        b"Fifth message".to_vec(),
    ];
    
    // Push all messages in a single batch operation
    let last_id = ddl.push_batch("test.batch", payloads).await?;
    
    println!("Successfully pushed batch of 5 messages, last ID: {}", last_id);
    
    // Process all messages with timeout
    let start = std::time::Instant::now();
    let mut count = 0;
    while count < 5 && start.elapsed().as_secs() < 5 {
        if let Some(entry) = stream.try_next() {
            count += 1;
            println!("Received entry #{}: {:?}", entry.id, String::from_utf8_lossy(&entry.payload));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    println!("Processed {} messages from batch", count);
    
    Ok(())
}