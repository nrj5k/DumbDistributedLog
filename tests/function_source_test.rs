use autoqueues::config::Config;
use autoqueues::queue::interval::IntervalConfig;
use autoqueues::queue::source::{FunctionSource, QueueSource};
use autoqueues::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tokio;

#[test]
fn test_function_source_creation() {
    // Test that we can create a FunctionSource
    let source = FunctionSource::new(|| 42);
    assert!(!source.is_paused());
    assert!(!source.should_stop());
}

#[tokio::test]
async fn test_function_source_pause_resume() {
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
    
    // Create a simple queue
    let queue = Arc::new(RwLock::new(SimpleQueue::<u32, 1024>::new()));
    
    // Create a counter to track function calls
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    let source = FunctionSource::new(move || {
        counter_clone.fetch_add(1, Ordering::Relaxed)
    });
    
    let config = Config::default();
    let interval_config = IntervalConfig::Constant(50); // 50ms interval for testing (more stable)
    
    // Start source, verify it produces data
    source.start(queue.clone(), &config, interval_config, None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_before = counter.load(Ordering::Relaxed);
    assert!(count_before > 0, "Should have generated some data initially");
    
    // Pause, verify no new data
    source.pause();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_during_pause = counter.load(Ordering::Relaxed);
    assert_eq!(count_during_pause, count_before, "Should not increment while paused");
    
    // Resume, verify data resumes
    source.resume();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_after = counter.load(Ordering::Relaxed);
    assert!(count_after > count_during_pause, "Should increment after resume");
}

#[tokio::test]
async fn test_function_source_remove() {
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
    
    // Create a simple queue
    let queue = Arc::new(RwLock::new(SimpleQueue::<u32, 1024>::new()));
    
    // Create a counter to track function calls
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    let source = FunctionSource::new(move || {
        counter_clone.fetch_add(1, Ordering::Relaxed)
    });
    
    let config = Config::default();
    let interval_config = IntervalConfig::Constant(50); // 50ms interval for testing (more stable)
    
    // Start source, verify it produces data
    source.start(queue.clone(), &config, interval_config, None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_before = counter.load(Ordering::Relaxed);
    assert!(count_before > 0, "Should have generated some data initially");
    
    // Remove/stop the source
    source.remove();
    assert!(source.should_stop(), "Source should be marked for stopping");
    
    // Wait a bit and verify no significant new data is produced (allowing for race conditions)
    let count_when_removed = counter.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_after_removal = counter.load(Ordering::Relaxed);
    // Allow for at most 2 additional increments due to race conditions
    assert!(count_after_removal - count_when_removed <= 2, "Should not increment much after removal");
}

#[tokio::test]
async fn test_function_source_integration() {
    // Create a simple queue
    let queue = Arc::new(RwLock::new(SimpleQueue::<u32, 1024>::new()));
    
    // Create a function source that generates incrementing numbers
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    let source = FunctionSource::new(move || {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    });
    
    // Start the source
    let config = Config::default();
    let interval_config = IntervalConfig::Constant(5); // 5ms interval for fast testing
    source.start(queue.clone(), &config, interval_config, None);
    
    // Let it run for a bit to generate some data
    tokio::time::sleep(Duration::from_millis(30)).await;
    
    // Check that it was producing data
    let initial_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    assert!(initial_count > 0, "Should have generated some data");
    
    // Pause the source
    source.pause();
    
    // Let it sit paused for a while
    let paused_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(30)).await;
    
    // Make sure it didn't increment much while paused (allowing for race conditions)
    let after_pause_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    assert!(after_pause_count - paused_count <= 2, "Should not increment much while paused");
    
    // Resume the source
    source.resume();
    
    // Capture count before resuming
    let resume_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    
    // Let it run again
    tokio::time::sleep(Duration::from_millis(30)).await;
    
    // Make sure it incremented again
    let resumed_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    assert!(resumed_count > resume_count, "Should increment after resume");
    
    // Remove/stop the source
    source.remove();
    
    // Let it sit stopped for a while
    let stopped_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(30)).await;
    
    // Make sure it didn't increment after being removed (allowing for race conditions)
    let final_count = counter.load(std::sync::atomic::Ordering::Relaxed);
    assert!(final_count - stopped_count <= 2, "Should not increment much after removal");
}
