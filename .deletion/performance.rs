//! Performance benchmarks for autoqueues library
//!
//! These benchmarks measure the performance characteristics of the queue system.

use autoqueues::*;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn bench_publish_performance() -> Result<(), Box<dyn std::error::Error>> {
    let hook: FunctionHook<i32> = Arc::new(|_| Ok(42i32));
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        1000,
        2,
        "bench_publish.log".to_string(),
        "bench_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let mut queue = Queue::new(config);

    let start = Instant::now();
    let iterations = 10000;

    for i in 0..iterations {
        queue.publish(i).await?;
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "Published {} items in {:?} ({:.0} items/sec)",
        iterations, elapsed, rate
    );

    // Verify queue has data
    let stats = queue.get_stats();
    assert_eq!(stats.size, iterations as usize);

    // Should be reasonably fast
    assert!(rate > 1000.0, "Publish rate should be > 1000 items/sec");

    Ok(())
}

#[tokio::test]
async fn bench_concurrent_publishers() -> Result<(), Box<dyn std::error::Error>> {
    let hook: FunctionHook<i32> = Arc::new(|_| Ok(42i32));
    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        500,
        2,
        "bench_concurrent.log".to_string(),
        "bench_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let queue = Arc::new(tokio::sync::Mutex::new(Queue::new(config)));

    let start = Instant::now();
    let mut handles = Vec::new();
    let publishers = 10;
    let items_per_publisher = 1000;

    for pub_id in 0..publishers {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            for i in 0..items_per_publisher {
                let mut q = queue_clone.lock().await;
                if let Err(e) = q.publish((pub_id * items_per_publisher + i) as i32).await {
                    eprintln!("Publish error: {}", e);
                }
                drop(q); // Release lock

                // Small yield occasionally
                if i % 50 == 0 {
                    tokio::task::yield_now().await;
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all publishers
    for handle in handles {
        handle.await?;
    }

    let elapsed = start.elapsed();
    let total_items = publishers * items_per_publisher;
    let rate = total_items as f64 / elapsed.as_secs_f64();

    println!(
        "Concurrent: {} publishers × {} items in {:?} ({:.0} items/sec)",
        publishers, items_per_publisher, elapsed, rate
    );

    // Verify data was published
    let final_queue = queue.lock().await;
    let final_stats = final_queue.get_stats();
    assert!(final_stats.size > 0);

    // Should still be reasonably fast with concurrency
    assert!(
        rate > 500.0,
        "Concurrent publish rate should be > 500 items/sec"
    );

    Ok(())
}

#[tokio::test]
async fn bench_server_overhead() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = counter.clone();

    let hook: FunctionHook<i32> = Arc::new(move |_| {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(42)
    });

    let queue_type = QueueType::new(
        QueueValue::ClusterAvailability,
        50, // Fast interval for benchmarking
        2,
        "bench_server.log".to_string(),
        "bench_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let queue = Queue::new(config);

    // Benchmark with server
    let start = Instant::now();
    let handle = queue.start_server()?;

    // Let server run
    let runtime = Duration::from_secs(1);
    sleep(runtime).await;

    handle.shutdown().await?;
    let elapsed = start.elapsed();

    let server_calls = counter.load(std::sync::atomic::Ordering::Relaxed);
    let rate = server_calls as f64 / elapsed.as_secs_f64();

    println!(
        "Server: {} calls in {:?} ({:.0} calls/sec)",
        server_calls, elapsed, rate
    );

    // Server should be making regular calls
    assert!(server_calls > 0, "Server should have called the hook");
    assert!(rate > 10.0, "Server call rate should be > 10 calls/sec");

    Ok(())
}

#[tokio::test]
async fn bench_memory_usage() -> Result<(), Box<dyn std::error::Error>> {
    let hook: FunctionHook<i64> = Arc::new(|_| Ok(123456789i64));
    let queue_type = QueueType::new(
        QueueValue::Sim,
        100,
        2,
        "bench_memory.log".to_string(),
        "bench_var".to_string(),
    );

    let config = QueueConfig::new(Mode::Sensor, hook, Model::Linear, queue_type);
    let mut queue = Queue::new(config);

    // Fill with a lot of data
    let start = Instant::now();
    let large_dataset = 50000;

    for i in 0..large_dataset {
        queue.publish(i).await?;
    }

    let elapsed = start.elapsed();
    let stats = queue.get_stats();

    println!(
        "Memory usage: {} items stored in {:?} ({} bytes per item avg)",
        stats.size,
        elapsed,
        if stats.size > 0 {
            large_dataset as u64 * 8 / stats.size as u64
        } else {
            0
        }
    );

    // Should handle large datasets efficiently
    assert!(stats.size > 0);
    assert!(
        elapsed.as_secs() < 10,
        "Large dataset should be processed quickly"
    );

    Ok(())
}
