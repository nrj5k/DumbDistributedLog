//! Basic usage example for the AutoQueues API

use autoqueues::AutoQueues;

fn get_cpu_usage() -> f64 {
    // In a real implementation, this would get actual CPU usage
    42.0
}

fn get_memory_usage() -> f64 {
    // In a real implementation, this would get actual memory usage
    60.0
}

fn main() {
    // Configure AutoQueues
    let config = AutoQueues::configure()
        .nodes(&["node1:6969", "node2:6969"])
        .discovery(autoqueues::autoqueues_config::DiscoveryMethod::Static)
        .capacity(1024)
        .default_interval(1000);

    // Create AutoQueues instance
    let autoqueues = AutoQueues::new(config);

    // Add a simple function-based queue for CPU
    autoqueues
        .add_queue_fn::<f64, _>("cpu", get_cpu_usage)
        .expect("Failed to add CPU queue");

    // Add a function-based queue for memory
    autoqueues
        .add_queue_fn::<f64, _>("memory", get_memory_usage)
        .expect("Failed to add memory queue");

    // Start all queues
    autoqueues.start();

    println!("AutoQueues initialized successfully!");
}
