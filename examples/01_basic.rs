//! AutoQueues Basic Example

fn main() {
    println!("AutoQueues Example\n");
    
    let config = AutoQueues::configure()
        .nodes(&["node1:6969"])
        .capacity(1024)
        .default_interval(1000);
    
    let queues = AutoQueues::new(config);
    println!("Configured");
    
    // Function-based queue (auto-populates every 1000ms)
    queues.add_queue_fn::<f64, _>("cpu", || 42.0);
    println!("Created cpu queue");
    
    // Start queues
    queues.start();
    println!("Started");
    
    // Pop data (synchronous)
    match queues.try_pop::<f64>("cpu") {
        Ok(Some(cpu)) => println!("CPU: {:.1}%", cpu),
        Ok(None) => println!("CPU queue empty"),
        Err(e) => println!("Error: {}", e),
    }
    
    println!("Done");
}
