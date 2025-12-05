use autoqueues::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔬 Autoqueues Performance Benchmarks");
    println!("==================================================");
    
    // Basic configuration that works with current API
    benchmark_throughput().await?;
    benchmark_latency().await?;
    benchmark_memory_usage().await?;
    
    println!("\n✅ All benchmarks completed!");
    Ok(())
}

async fn benchmark_throughput() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Throughput Benchmark");
    println!("-------------------------");

    let test_configs = vec![
        ("Small batches", 10, 10),    // 10 items, 10ms intervals
        ("Medium batches", 100, 10),   // 100 items, 10ms intervals  
        ("Large batches", 1000, 10),    // 1000 items, 10ms intervals
    ];

    for (name, batch_size, interval_ms) in test_configs {
        println!("\n🔄 Testing {} (batch_size={}, interval={}ms)...", name, batch_size, interval_ms);
        
        let queue_type = QueueType::new(
            QueueValue::Sim,
            interval_ms as u64,
            2,
            format!("throughput_{}.log", batch_size),
            format!("throughput_{}_var", batch_size),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(|_| Ok(42)), // Fixed value
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config.clone());
        
        let start = Instant::now();
        
        // Manually publish batch
        for i in 0..batch_size {
            queue.publish(i).await?;
        }
        
        let publish_time = start.elapsed();
        let throughput = batch_size as f64 / publish_time.as_secs_f64();
        
        println!("✅ Published {} items in {:?}", batch_size, publish_time);
        println!("📈 Throughput: {:.0} items/second", throughput);
    }

    Ok(())
}

async fn benchmark_latency() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⏱️  Latency Benchmark");
    println!("--------------------");

    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        100,  // 100ms base interval
        3,
        "latency_test.log".to_string(),
        "latency_test_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(|_| Ok(42)), // Fixed value
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config.clone());

    // Measure publish latency
    let iterations = 100;
    let mut total_publish_time = Duration::ZERO;
    
    println!("🎯 Measuring publish latency ({} iterations)...", iterations);
    
    for i in 0..iterations {
        let start = Instant::now();
        queue.publish(i).await?;
        total_publish_time += start.elapsed();
    }
    
    let avg_publish_latency = total_publish_time / iterations;
    println!("✅ Average publish latency: {:?}", avg_publish_latency);

    Ok(())
}

async fn benchmark_memory_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧠 Memory Usage Benchmark");
    println!("------------------------");

    let test_sizes = vec![100, 1000, 10000];
    
    for size in test_sizes {
        println!("\n💾 Testing with {} items...", size);
        
        let queue_type = QueueType::new(
            QueueValue::NodeCapacity,
            50,  // Fast 50ms intervals
            5,
            format!("memory_{}.log", size),
            format!("memory_{}_var", size),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(|_| Ok(99)),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config.clone());
        
        // Fill to capacity
        for i in 0..size {
            queue.publish(i).await?;
        }
        
        let stats = queue.get_stats();
        println!("✅ Queue size: {} (capacity: {})", 
                 stats.size, stats.capacity); 
    }

    Ok(())
}