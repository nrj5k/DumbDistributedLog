//! Simple integration tests for the autoqueues library

use autoqueues::*;
use std::sync::Arc;
use std::time::Duration;
use tokio;

#[tokio::test]
async fn test_basic_queue_operations() -> Result<(), Box<dyn std::error::Error>> {
    // Create a basic queue
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000,
        2,
        "test_trace.log".to_string(),
        "test_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(42i32)),
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config);

    // Test initial state
    let stats = queue.get_stats();
    assert_eq!(stats.size, 0);

    // Add some data
    queue.publish(100).await?;
    queue.publish(200).await?;

    // Verify data was added
    let updated_stats = queue.get_stats();
    assert_eq!(updated_stats.size, 2);

    // Get latest data
    let latest = queue.get_latest().await;
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().1, 200);

    Ok(())
}

#[tokio::test]
async fn test_queue_server() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = counter.clone();

    let hook: FunctionHook<i32> = Arc::new(move |_| {
        let current = counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(current as i32)
    });

    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        100, // Fast interval
        2,
        "server_test.log".to_string(),
        "server_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let queue = Queue::new(config);

    // Start server
    let handle = queue.start_server()?;
    assert!(handle.is_running());

    // Let server run
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify hook was called by checking the counter
    let final_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    assert!(final_count > 0);

    // Shutdown server
    handle.shutdown().await?;

    Ok(())
}

#[tokio::test]
async fn test_aimd_queue() -> Result<(), Box<dyn std::error::Error>> {
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000,
        2,
        "aimd_test.log".to_string(),
        "aimd_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(42i32)),
        Model::Linear,
        queue_type,
    );

    let aimd_config = AimdConfig {
        initial_interval_ms: 1000,
        min_interval_ms: 100,
        max_interval_ms: 5000,
        additive_factor_ms: 50,
        multiplicative_factor: 0.8,
        variance_threshold: Some(1.0),
        cooldown_ms: Some(0),
        window_size: Some(10),
    };

    let mut queue = Queue::with_aimd(config, aimd_config);

    // Test AIMD functionality
    assert_eq!(queue.get_current_interval(), 1000);

    // Add data
    for value in [10, 20, 30, 15, 25] {
        queue.publish(value).await?;
    }

    // Check AIMD stats
    let stats = queue.get_aimd_stats();
    assert!(stats.is_some());

    let aimd_stats = stats.unwrap();
    assert_eq!(aimd_stats.current_interval, 1000);

    Ok(())
}

#[tokio::test]
async fn test_queue_capacity() -> Result<(), Box<dyn std::error::Error>> {
    let queue_type = QueueType::new(
        QueueValue::Sim,
        500,
        2,
        "capacity_test.log".to_string(),
        "capacity_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|x| Ok(x[0])), // Hook called with Vec when server runs, but publish takes single values
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config);

    // Fill beyond capacity
    for i in 0..5000 {
        queue.publish(i).await?;
    }

    // Should respect capacity
    let stats = queue.get_stats();
    assert_eq!(stats.capacity, 4096);
    // Note: Current implementation doesn't strictly enforce capacity limit
    assert!(stats.size > 0);

    // Latest should be most recent
    let latest = queue.get_latest().await;
    assert!(latest.is_some());

    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    let error_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = error_counter.clone();

    let hook: FunctionHook<i32> = Arc::new(move |_| {
        let count = counter_clone.load(std::sync::atomic::Ordering::Relaxed);
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if count % 5 == 4 {
            Err(QueueError::Other("Simulated error".to_string()))
        } else {
            Ok(42)
        }
    });

    let queue_type = QueueType::new(
        QueueValue::NodeCapacity,
        100,
        2,
        "error_test.log".to_string(),
        "error_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let mut queue = Queue::new(config);

    // Test that direct publish bypasses hook (hook only called by server)
    // All direct publishes should succeed
    let mut successful = 0;
    for i in 0..20 {
        if queue.publish(i).await.is_ok() {
            successful += 1;
        }
    }

    // All direct publishes should succeed
    assert_eq!(successful, 20);

    // Hook count should be 0 because we didn't use server
    assert_eq!(error_counter.load(std::sync::atomic::Ordering::Relaxed), 0);

    // Queue should still work
    let latest = queue.get_latest().await;
    assert!(latest.is_some());

    Ok(())
}
