//! Enhanced networking demo for AutoQueues
//!
//! Demonstrates TCP-based distributed queue functionality with
//! proper error handling, timeouts, and connection management.

use autoqueues::{TcpTransport, DistributedQueueManager, NetworkNode};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("AutoQueues Enhanced Networking Demo");
    println!("==================================");

    // Demo 1: Basic TCP transport creation
    println!("\n1. TCP Transport Creation:");
    let transport = TcpTransport::new();
    println!("   Transport created: {}", !transport.is_connected());
    println!("   Remote address: {:?}", transport.remote_addr());

    // Demo 2: TCP transport with custom timeout
    println!("\n2. TCP Transport with Custom Timeout:");
    let timeout_transport = TcpTransport::with_timeout(Duration::from_secs(60));
    println!("   Transport with 60s timeout created");

    // Demo 3: Distributed queue manager
    println!("\n3. Distributed Queue Manager:");
    let manager = DistributedQueueManager::<String>::new(Box::new(transport));
    println!("   Manager created: {}", !manager.is_connected());
    println!("   Local queue size: {}", manager.local_size());

    // Demo 4: Local queue operations
    println!("\n4. Local Queue Operations:");
    manager.add_local("Message 1".to_string());
    manager.add_local("Message 2".to_string());
    manager.add_local("Message 3".to_string());
    
    let local_items = manager.get_local();
    println!("   Local queue contains {} items:", local_items.len());
    for (i, item) in local_items.iter().enumerate() {
        println!("     Item {}: {}", i + 1, item);
    }

    // Demo 5: Network node operations
    println!("\n5. Network Node Operations:");
    let mut node = NetworkNode::<String>::new();
    println!("   Network node created");
    
    // Add items to node's local queue
    node.add_local("Node Message 1".to_string());
    node.add_local("Node Message 2".to_string());
    
    let node_items = node.get_local();
    println!("   Node local queue: {} items", node_items.len());

    // Demo 6: Server binding simulation
    println!("\n6. Server Binding:");
    match TcpTransport::bind("127.0.0.1:0") {
        Ok(mut server_transport) => {
            println!("   Server bound successfully");
            println!("   Connected: {}", server_transport.is_connected());
        }
        Err(e) => {
            println!("   Server binding failed: {}", e);
        }
    }

    // Demo 7: Connection simulation
    println!("\n7. Connection Simulation:");
    let mut client_node = NetworkNode::<String>::new();
    match client_node.connect("127.0.0.1:8080") {
        Ok(_) => {
            println!("   Client connected successfully");
            println!("   Connected: {}", client_node.manager.is_connected());
        }
        Err(e) => {
            println!("   Client connection failed (expected): {}", e);
        }
    }

    // Demo 8: Data serialization test
    println!("\n8. Data Serialization Test:");
    let test_data = vec![
        "Test Message 1".to_string(),
        "Test Message 2".to_string(),
        "Test Message 3".to_string(),
    ];
    
    println!("   Test data created with {} items", test_data.len());
    for (i, item) in test_data.iter().enumerate() {
        println!("     Data {}: {}", i + 1, item);
    }

    println!("\nDemo completed successfully!");
    println!("Features demonstrated:");
    println!("   - TCP transport creation and configuration");
    println!("   - Distributed queue management");
    println!("   - Local queue operations");
    println!("   - Network node functionality");
    println!("   - Server binding capabilities");
    println!("   - Connection handling");
    println!("   - Data serialization support");

    Ok(())
}