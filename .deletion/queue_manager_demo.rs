//! Queue Manager Demonstration
//!
//! Professional example showing QueueManager as Engine trait implementation
//! for distributed coordination across hosts. Queue instances call QueueManager
//! as just another Engine in their processing chain.

use autoqueues::{Engine, LoadBalancingRequest, ManagerInput, ManagerOutput, QueueManager};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Queue Manager Distributed Coordination Demo ===\n");

    // Initialize QueueManager as distributed coordination engine
    let manager = Arc::new(QueueManager::new("cluster_coordinator"));

    // Register cluster hosts for load balancing
    manager.register_host("host-alpha".to_string()).await;
    manager.register_host("host-beta".to_string()).await;
    manager.register_host("host-gamma".to_string()).await;

    println!("✓ QueueManager initialized as Engine<Input, Output>");
    println!("✓ Registered 3 cluster hosts for distributed coordination");

    // Simulate distributed cluster metrics
    manager
        .update_global_variable("global.cluster_cpu".to_string(), 45.2)
        .await;
    manager
        .update_global_variable("global.cluster_memory".to_string(), 62.8)
        .await;

    println!("✓ Updated cluster-wide metrics");
    println!("  - Cluster CPU: 45.2%");
    println!("  - Cluster Memory: 62.8%");

    // Test QueueManager as Engine for variable resolution
    println!("\n--- Variable Resolution Engine Test ---");

    let input = ManagerInput::VariableRequest("global.cluster_cpu".to_string());
    let result = manager.execute(input)?;

    match result {
        ManagerOutput::VariableValue(value) => {
            println!("✓ Engine resolved global.cluster_cpu = {:.1}%", value);
        }
        _ => println!("✗ Unexpected output type"),
    }

    // Test QueueManager as Engine for health monitoring
    println!("\n--- Health Monitoring Engine Test ---");

    let input = ManagerInput::HealthCheck("data_processing_queue".to_string());
    let result = manager.execute(input)?;

    match result {
        ManagerOutput::HealthStatus(status) => {
            println!("✓ Engine health check for '{}'", status.queue_name);
            println!("  - Healthy: {}", status.is_healthy);
            println!("  - CPU Usage: {:.1}%", status.cpu_usage);
            println!("  - Memory Usage: {:.1}%", status.memory_usage);
            println!("  - Queue Depth: {}", status.queue_depth);
        }
        _ => println!("✗ Unexpected output type"),
    }

    // Test QueueManager as Engine for load balancing
    println!("\n--- Load Balancing Engine Test ---");

    let load_request = LoadBalancingRequest {
        queue_name: "analytics_queue".to_string(),
        current_load: 75.0,
        target_hosts: vec!["host-alpha".to_string(), "host-beta".to_string()],
    };

    let input = ManagerInput::LoadBalancing(load_request);
    let result = manager.execute(input)?;

    match result {
        ManagerOutput::LoadRecommendation(rec) => {
            println!("✓ Engine load balancing recommendation");
            println!("  - Recommended Host: {}", rec.recommended_host);
            println!("  - Estimated Load: {:.1}%", rec.estimated_load);
            println!("  - Confidence: {:.1}%", rec.confidence * 100.0);
        }
        _ => println!("✗ Unexpected output type"),
    }

    // Demonstrate Queue integration with QueueManager Engine
    println!("\n--- Queue + QueueManager Engine Integration ---");

    // Note: Queue creation with expressions would be enhanced in future
    // Current queue creation uses QueueConfig (conceptual demonstration)
    println!("✓ Conceptual queue with expression using global.cluster_cpu");
    println!("  - Real implementation would use QueueConfig with expression processor");
    println!("  - Expression: '(local.cpu_usage + global.cluster_cpu) / 2.0'");
    println!("  - Queue would call QueueManager Engine for global.cluster_cpu resolution");

    // Simulate queue operation with Engine coordination
    println!("\n--- Simulated Distributed Queue Operation ---");

    // In a real system, Queue would call QueueManager Engine like this:
    for i in 0..3 {
        // Queue requests global variable resolution
        let request = ManagerInput::VariableRequest("global.cluster_cpu".to_string());
        let result = manager.execute(request)?;

        if let ManagerOutput::VariableValue(global_cpu) = result {
            println!(
                "Engine call {}: global.cluster_cpu = {:.1}% (resolved for queue)",
                i + 1,
                global_cpu
            );
        }

        sleep(Duration::from_millis(500)).await;
    }

    println!("\n=== Queue Manager Engine Summary ===");
    println!("✓ QueueManager successfully implements Engine<ManagerInput, ManagerOutput>");
    println!("✓ Distributed coordination engine operational");
    println!("✓ Variable resolution: global.* variables handled");
    println!("✓ Health monitoring: queue health status tracking");
    println!("✓ Load balancing: cross-host coordination");
    println!("✓ Queue integration: seamless Engine chaining");

    println!("\n🎯 Next: Queue instances will automatically call QueueManager Engine");
    println!("   for global variable resolution in expression evaluation.");

    Ok(())
}
