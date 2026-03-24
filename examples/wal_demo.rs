//! WAL Demo - Shows how DDL with WAL durability works
//!
//! This demonstrates the Write-Ahead Log feature that ensures
//! entries survive crashes by writing to disk before acknowledging.

use ddl::{DdlWithWal, DdlConfig, DDL, EntryStream};
use ddl::traits::ddl::BackpressureMode;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create temporary directory for data
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path();
    
    println!("=== DDL WAL Durability Demo ===\n");
    
    // Create DDL config with WAL enabled
    let config = DdlConfig {
        node_id: 1,
        peers: vec![],
        owned_topics: vec!["demo.metrics".to_string()],
        buffer_size: 1024,
        max_topics: 10000, // Default limit of 10,000 topics
        gossip_enabled: false,
        gossip_bind_addr: "0.0.0.0:0".to_string(),
        gossip_bootstrap: vec![],
        data_dir: data_dir.to_path_buf(),
        wal_enabled: true,
        subscription_buffer_size: 1000,
        subscription_backpressure: BackpressureMode::DropOldest,
    };
    
    // Create DDL with WAL
    let ddl = DdlWithWal::new(config, data_dir)?;
    
    // Push some entries
    println!("Pushing entries to WAL-enabled DDL...");
    
    let payloads = vec![
        br#"{"metric": "cpu_usage", "value": 42.5}"#.to_vec(),
        br#"{"metric": "memory_usage", "value": 67.2}"#.to_vec(),
        br#"{"metric": "disk_io", "value": 12.8}"#.to_vec(),
    ];
    
    for (i, payload) in payloads.into_iter().enumerate() {
        let entry_id = ddl.push("demo.metrics", payload).await?;
        println!("  Pushed entry #{} with ID: {}", i, entry_id);
    }
    
    // Subscribe and read entries back
    println!("\nSubscribing to topic and reading entries...");
    let mut stream: EntryStream = ddl.subscribe("demo.metrics").await?;
    
    while let Some(entry) = stream.next() {
        println!("  Received entry #{}: {:?}", entry.id, String::from_utf8_lossy(&entry.payload));
    }
    
    // Show WAL persistence
    println!("\nWAL files are persisted at: {:?}", data_dir.join("wal"));
    
    println!("\nDemo completed successfully!");
    println!("Entries are durably stored in WAL and will survive process restarts.");
    
    Ok(())
}