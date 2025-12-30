//! Comprehensive example demonstrating advanced usage of the LockFreeQueue
//!
//! This example shows how to use the LockFreeQueue with different data types
//! and demonstrates its performance characteristics.

use autoqueues::queue::lockfree::LockFreeQueue;
use autoqueues::QueueTrait;
use std::time::Instant;

fn main() {
    println!("Comprehensive LockFreeQueue Example");
    println!("==================================");
    
    // Example 1: Basic integer queue
    println!("\n1. Basic Integer Queue:");
    basic_integer_queue_example();
    
    // Example 2: String queue
    println!("\n2. String Queue:");
    string_queue_example();
    
    // Example 3: Custom struct queue
    println!("\n3. Custom Struct Queue:");
    custom_struct_queue_example();
    
    // Example 4: Performance test
    println!("\n4. Performance Test:");
    performance_test();
    
    println!("\nAll examples completed successfully!");
}

fn basic_integer_queue_example() {
    let mut queue: LockFreeQueue<i32, 16> = LockFreeQueue::new();
    
    // Add some integers
    for i in 1..=10 {
        queue.publish(i).expect("Failed to publish");
    }
    
    println!("   Queue size: {}", queue.get_size());
    println!("   Is full: {}", queue.is_full());
    
    // Get latest 5 items
    let recent = queue.get_latest_n(5);
    println!("   Latest 5 items: {:?}", recent);
    
    // Get the very latest
    if let Some((_, value)) = queue.get_latest() {
        println!("   Most recent value: {}", value);
    }
}

fn string_queue_example() {
    let mut queue: LockFreeQueue<String, 8> = LockFreeQueue::new();
    
    // Add some strings
    let messages = vec![
        "Hello, World!".to_string(),
        "Lock-free queue".to_string(),
        "High performance".to_string(),
        "Zero mutex overhead".to_string(),
    ];
    
    for msg in messages {
        queue.publish(msg).expect("Failed to publish");
    }
    
    println!("   Queue size: {}", queue.get_size());
    
    // Get all items
    let all_messages = queue.get_latest_n(10);
    for (i, msg) in all_messages.iter().enumerate() {
        println!("   Message {}: {}", i + 1, msg);
    }
}

#[derive(Clone, Debug)]
struct SensorData {
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

fn custom_struct_queue_example() {
    let mut queue: LockFreeQueue<SensorData, 8> = LockFreeQueue::new();
    
    // Add some sensor data
    let sensor_readings = vec![
        SensorData { temperature: 23.5, humidity: 45.2, timestamp: 1000 },
        SensorData { temperature: 24.1, humidity: 46.8, timestamp: 1001 },
        SensorData { temperature: 23.8, humidity: 45.9, timestamp: 1002 },
        SensorData { temperature: 24.3, humidity: 47.1, timestamp: 1003 },
    ];
    
    for reading in sensor_readings {
        queue.publish(reading).expect("Failed to publish");
    }
    
    println!("   Queue size: {}", queue.get_size());
    
    // Get the latest reading
    if let Some((_, latest)) = queue.get_latest() {
        println!("   Latest reading: {:.1}°C, {:.1}% humidity", 
                 latest.temperature, latest.humidity);
    }
    
    // Get all readings
    let all_readings = queue.get_latest_n(10);
    println!("   All readings:");
    for (i, reading) in all_readings.iter().enumerate() {
        println!("     {}: {:.1}°C, {:.1}% humidity (ts: {})", 
                 i + 1, reading.temperature, reading.humidity, reading.timestamp);
    }
}

fn performance_test() {
    const COUNT: usize = 1000;
    
    let mut queue: LockFreeQueue<i32, 1024> = LockFreeQueue::new();
    
    // Measure publish performance
    let start = Instant::now();
    for i in 0..COUNT {
        queue.publish(i as i32).expect("Failed to publish");
    }
    let publish_duration = start.elapsed();
    
    // Measure get_latest performance
    let start = Instant::now();
    for _ in 0..COUNT {
        let _ = queue.get_latest();
    }
    let get_latest_duration = start.elapsed();
    
    // Measure get_latest_n performance
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = queue.get_latest_n(10);
    }
    let get_latest_n_duration = start.elapsed();
    
    println!("   Published {} items in: {:?}", COUNT, publish_duration);
    println!("   get_latest called {} times in: {:?}", COUNT, get_latest_duration);
    println!("   get_latest_n(10) called 1000 times in: {:?}", get_latest_n_duration);
    
    let publish_per_sec = (COUNT as f64) / publish_duration.as_secs_f64();
    let get_latest_per_sec = (COUNT as f64) / get_latest_duration.as_secs_f64();
    
    println!("   Publish rate: {:.0} ops/sec", publish_per_sec);
    println!("   Get latest rate: {:.0} ops/sec", get_latest_per_sec);
}