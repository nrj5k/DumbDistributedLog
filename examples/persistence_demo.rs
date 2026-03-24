//! Persistence Demo
//!
//! Demonstrates the persistence functionality in DDL.

use ddl::ddl_wal::DdlWithWal;
use ddl::traits::ddl::{DDL, DdlConfig};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting DDL with persistence demo...");
    
    // Create temporary directory for data
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path();
    
    // Create a config with WAL enabled
    let mut config = DdlConfig::default();
    config.wal_enabled = true;
    config.data_dir = data_dir.to_path_buf();
    config.owned_topics = vec!["cpu_monitor".to_string(), "cpu_alert".to_string()];
    
    // Create DDL instance with WAL
    let ddl = DdlWithWal::new(config, data_dir)?;
    
    // Push some entries
    println!("Pushing entries to WAL-enabled DDL...");
    
    let payloads = vec![
        br#"{"metric": "cpu_usage", "value": 42.5}"#.to_vec(),
        br#"{"metric": "memory_usage", "value": 67.2}"#.to_vec(),
        br#"{"metric": "disk_io", "value": 12.8}"#.to_vec(),
    ];
    
    for (i, payload) in payloads.into_iter().enumerate() {
        let entry_id = ddl.push("cpu_monitor", payload).await?;
        println!("  Pushed entry #{} with ID: {}", i, entry_id);
    }
    
    // Subscribe and read entries back
    println!("\nSubscribing to topic and reading entries...");
    match ddl.subscribe("cpu_monitor").await {
        Ok(stream) => {
            while let Some(entry) = stream.try_next() {
                println!("  Received entry #{}: {:?}", entry.id, String::from_utf8_lossy(&entry.payload));
                // Acknowledge entries
                match ddl.ack("cpu_monitor", entry.id).await {
                    Ok(_) => println!("    Acknowledged entry #{}", entry.id),
                    Err(e) => println!("    Error acknowledging entry #{}: {}", entry.id, e),
                }
            }
        },
        Err(e) => println!("Error subscribing: {}", e),
    }
    
    // Show WAL persistence
    println!("\nWAL files are persisted at: {:?}", data_dir.join("wal"));
    
    println!("\nDemo completed successfully!");
    println!("Entries are durably stored in WAL and will survive process restarts.");
    
    Ok(())
}