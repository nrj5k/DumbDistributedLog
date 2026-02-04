//! Basic usage example for the AutoQueues API

use autoqueues::config::ConfigGenerator;
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
    // Generate a local test config with 3 nodes
    let config = ConfigGenerator::local_test(3, 6967);

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
