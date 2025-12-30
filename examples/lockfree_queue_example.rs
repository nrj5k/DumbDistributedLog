//! Example demonstrating the usage of the LockFreeQueue

use autoqueues::queue::lockfree::LockFreeQueue;
use autoqueues::traits::queue::QueueTrait;

fn main() {
    println!("LockFreeQueue Example");
    println!("====================");
    
    // Create a new lock-free queue with capacity 8
    let mut queue: LockFreeQueue<i32, 8> = LockFreeQueue::new();
    
    // Test basic operations
    println!("1. Testing basic operations:");
    
    // Check initial state
    println!("   Initial size: {}", queue.get_size());
    println!("   Is empty: {}", queue.is_empty());
    println!("   Is full: {}", queue.is_full());
    
    // Add some items
    for i in 1..=5 {
        queue.publish(i).unwrap();
        println!("   Published: {}, Size: {}", i, queue.get_size());
    }
    
    // Get latest item
    if let Some((_, latest)) = queue.get_latest() {
        println!("   Latest item: {}", latest);
    }
    
    // Get latest N items
    let recent = queue.get_latest_n(3);
    println!("   Latest 3 items: {:?}", recent);
    
    // Test queue full behavior
    println!("\n2. Testing queue full behavior:");
    
    // Fill the rest of the queue
    for i in 6..=7 {
        queue.publish(i).unwrap();
        println!("   Published: {}, Size: {}", i, queue.get_size());
    }
    
    // Try to publish when full
    match queue.publish(8) {
        Ok(_) => println!("   Published 8 successfully"),
        Err(e) => println!("   Error publishing 8: {}", e),
    }
    
    // Show final state
    println!("   Final size: {}", queue.get_size());
    println!("   Is full: {}", queue.is_full());
    
    // Get all items
    let all_items = queue.get_latest_n(7);
    println!("   All items ({}): {:?}", all_items.len(), all_items);
    
    println!("\nExample completed successfully!");
}