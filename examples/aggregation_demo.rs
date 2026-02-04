//! Aggregation Demo
//!
//! This example demonstrates how to use the distributed aggregation system
//! in AutoQueues to compute global metrics across multiple nodes.

use autoqueues::autoqueues::AutoQueues;
use autoqueues::config::Config;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Aggregation Demo");

    // Create configuration
    let config = Config::default();

    // Create AutoQueues instance
    let autoqueues = AutoQueues::new(config);

    // Add local metric queues
    autoqueues.add_queue_fn("cpu_node1", || {
        // Simulate CPU usage data
        let cpu = 20.0 + (rand::random::<f64>() * 60.0);
        println!("Publishing CPU usage: {:.2}%", cpu);
        cpu
    })?;

    autoqueues.add_queue_fn("memory_node1", || {
        // Simulate memory usage data
        let memory = 30.0 + (rand::random::<f64>() * 50.0);
        println!("Publishing Memory usage: {:.2}%", memory);
        memory
    })?;

    // Add distributed aggregation queues (replaces add_global_aggregation)
    autoqueues.add_distributed_queue("global_cpu_avg", "avg", vec!["cpu_node1".to_string()])?;
    autoqueues.add_distributed_queue("global_cpu_max", "max", vec!["cpu_node1".to_string()])?;
    autoqueues.add_distributed_queue("global_cpu_min", "min", vec!["cpu_node1".to_string()])?;

    // Add an expression-based queue (alternative to add_global_expression_aggregation)
    autoqueues.add_queue_expr(
        "health_score",
        "(100 - source1) + (100 - source2)",
        "cpu_node1,memory_node1", // comma-separated source queues
        true,                     // trigger on push
        None,                     // no specific interval
    )?;

    // Start all queues
    autoqueues.start();

    println!("Queues started. Waiting for data...");

    // Run for 10 seconds
    thread::sleep(Duration::from_secs(10));

    // Try to read some aggregated values
    match autoqueues.pop::<f64>("global_cpu_avg") {
        Ok(Some(avg_cpu)) => println!("Average CPU across cluster: {:.2}%", avg_cpu),
        Ok(None) => println!("No aggregated CPU data yet"),
        Err(e) => println!("Error reading CPU aggregation: {}", e),
    }

    match autoqueues.pop::<f64>("health_score") {
        Ok(Some(health_score)) => println!("Health score: {:.2}", health_score),
        Ok(None) => println!("No health score data yet"),
        Err(e) => println!("Error reading health score: {}", e),
    }

    println!("Demo completed.");
    Ok(())
}
