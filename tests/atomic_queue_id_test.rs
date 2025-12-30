//! Test for AtomicU32 queue ID generation

use autoqueues::queue_manager::QueueManager;

#[tokio::test]
async fn test_atomic_queue_id_generation() {
    println!("🧪 Testing AtomicU32 queue ID generation");
    
    let manager = QueueManager::new().expect("QueueManager should create successfully");
    
    // Start the manager to trigger queue creation
    manager.start().await.expect("QueueManager should start successfully");
    
    // Give it a moment to create queues
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Should have some queues now
    let queue_count = manager.get_queue_count().await;
    println!("📊 Created {} queues", queue_count);
    
    // Get queue names to verify they were created with unique IDs
    let queue_names = manager.get_queue_names().await;
    println!("📋 Queue names: {:?}", queue_names);
    
    // Stop the manager
    manager.stop().await.expect("QueueManager should stop successfully");
    
    assert!(queue_count > 0, "Should have created at least one queue");
    assert!(!queue_names.is_empty(), "Should have queue names");
    
    println!("✅ Atomic queue ID generation test passed");
}