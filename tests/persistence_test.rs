//! Test for the persistence functionality

use autoqueues::config::Config;
use autoqueues::queue::persistence::{PersistenceConfig, QueuePersistence};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

#[test]
fn test_persistence_basic() {
    // Create a temporary directory for testing
    let test_dir = "./test_data_persistence";

    // Clean up any existing test data
    let _ = fs::remove_dir_all(test_dir);

    // Create persistence config
    let config = PersistenceConfig {
        data_dir: test_dir.into(),
        max_file_size: 1024 * 1024, // 1MB
        flush_interval_ms: 100,
        include_timestamp: true,
        compress_old: false,
    };

    // Create a queue persistence instance
    let persistence =
        QueuePersistence::new("test_queue", config).expect("Failed to create persistence");

    // Persist some data
    persistence.persist(42.5);
    persistence.persist(100.0);
    persistence.persist(0.0);

    // Give some time for the background thread to write
    thread::sleep(Duration::from_millis(200));

    // Check if the log file was created
    let log_file = format!("{}/test_queue.log", test_dir);
    assert!(Path::new(&log_file).exists(), "Log file should exist");

    // Read the file content
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    assert!(!content.is_empty(), "Log file should not be empty");

    // Check that data was written
    assert!(content.contains("42.5"), "Log should contain 42.5");
    assert!(content.contains("100."), "Log should contain 100");

    // Clean up
    let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_persistence_manager() {
    use autoqueues::queue::persistence::PersistenceManager;

    // Create a temporary directory for testing
    let test_dir = "./test_data_manager";

    // Clean up any existing test data
    let _ = fs::remove_dir_all(test_dir);

    // Create persistence config
    let config = PersistenceConfig {
        data_dir: test_dir.into(),
        max_file_size: 1024 * 1024, // 1MB
        flush_interval_ms: 100,
        include_timestamp: true,
        compress_old: false,
    };

    // Create persistence manager
    let manager = PersistenceManager::new(config);

    // Enable persistence for a queue
    let handle = manager
        .enable_for_queue("test_queue")
        .expect("Failed to enable persistence");

    // Persist some data
    handle.persist(123.456);

    // Give some time for the background thread to write
    thread::sleep(Duration::from_millis(200));

    // Check if the log file was created
    let log_file = format!("{}/test_queue.log", test_dir);
    assert!(Path::new(&log_file).exists(), "Log file should exist");

    // Get handle again - should return the same instance
    let handle2 = manager
        .get_handle("test_queue")
        .expect("Should get existing handle");

    // They should be the same instance
    assert!(
        std::ptr::eq(handle.as_ref(), handle2.as_ref()),
        "Handles should be the same instance"
    );

    // Clean up
    let _ = fs::remove_dir_all(test_dir);
}
