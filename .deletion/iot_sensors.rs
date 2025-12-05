//! # IoT Sensor Data Processing Example
//!
//! This example demonstrates how to use the autoqueues library to process IoT sensor data
//! with autonomous queue management. It simulates a temperature monitoring system
//! with multiple sensors sending data at different intervals.

use autoqueues::{Queue, QueueConfig, QueueType, Mode, Model, QueueData, QueueError, QueueOperations, AimdQueue};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};

/// Sensor reading data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReading {
    sensor_id: String,
    temperature: f64,
    humidity: f64,
    battery_level: f64,
    location: (f64, f64), // (latitude, longitude)
}

/// Simulates sensor data generation
fn generate_sensor_data(sensor_id: &str) -> SensorReading {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();
    sensor_id.hash(&mut hasher);
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    timestamp.hash(&mut hasher);
    let hash = hasher.finish();
    
    SensorReading {
        sensor_id: sensor_id.to_string(),
        temperature: 18.0 + (hash % 100) as f64 * 0.07, // 18.0-25.0 range
        humidity: 40.0 + (hash % 100) as f64 * 0.30,    // 40.0-70.0 range
        battery_level: 20.0 + (hash % 100) as f64 * 0.80, // 20.0-100.0 range
        location: (
            -90.0 + (hash % 1000) as f64 * 0.18, // -90.0 to 90.0
            -180.0 + (hash % 2000) as f64 * 0.18, // -180.0 to 180.0
        ),
    }
}

/// Data processing hook - transforms sensor data before queueing
fn process_sensor_data(history: Vec<SensorReading>) -> Result<SensorReading, QueueError> {
    let sensor_id = format!("sensor_{}", std::time::SystemTime::now().elapsed().unwrap().subsec_millis() % 5);
    let mut reading = generate_sensor_data(&sensor_id);
    
    // Add some intelligence: if battery is low, adjust reading frequency
    if reading.battery_level < 30.0 {
        reading.temperature *= 0.95; // Simulate reduced accuracy
    }
    
    Ok(reading)
}

/// Main IoT processing system
async fn run_iot_system() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌡️  Starting IoT Sensor Data Processing System");
    println!("{}", "=".repeat(50));

    // Configure queue for IoT data
    let queue_config = QueueConfig {
        queue_type: QueueType::Circular {
            capacity: 1000,
            base_interval: 1000, // 1 second base interval
        },
        mode: Mode::default(),
        model: Model::default(),
        hook: std::sync::Arc::new(process_sensor_data),
    };

    // Create queue with AIMD for adaptive processing
    let aimd_config = autoqueues::AimdConfig {
        initial_interval_ms: 1000,
        min_interval_ms: 100,   // 100ms minimum
        max_interval_ms: 5000,  // 5 seconds maximum
        additive_factor_ms: 100,
        multiplicative_factor: 0.8,
        variance_threshold: Some(1.0),
        cooldown_ms: Some(2000),
        window_size: Some(10),
    };

    let mut queue = Queue::with_aimd(queue_config, aimd_config);
    
    // Start the autonomous server
    let mut server_handle = queue.start_server()?;
    println!("✅ IoT Server started with adaptive interval management");

    // Keep the system running for demonstration
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    
    for i in 0..12 { // Run for 60 seconds (12 * 5 seconds)
        interval.tick().await;
        
        // Get latest reading
        if let Some(latest) = queue.get_latest().await {
            println!("📊 Latest sensor data: {:?}", latest.1);
            
            // Get queue statistics
            let stats = queue.get_stats();
            println!("📈 Queue stats: size={}, capacity={}, interval={}ms", 
                     stats.size, stats.capacity, stats.interval_ms);
            
            // Get AIMD statistics if available
            if let Some(aimd_stats) = queue.get_aimd_stats() {
                println!("🎯 AIMD stats: current_interval={}ms, var={:.2}", 
                         aimd_stats.current_interval, aimd_stats.current_variance);
            }
        }
        
        // Demonstrate manual data publishing
        if i % 3 == 0 {
            let manual_reading = generate_sensor_data("manual_sensor");
            queue.publish(manual_reading).await?;
            println!("📝 Published manual sensor reading");
        }
    }

    println!("\n🛑 Shutting down IoT system...");
    server_handle.shutdown().await?;
    println!("✅ IoT system shutdown complete");

    // Demonstrate data retrieval
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;
    let past_30_seconds = current_time - 30_000;
    
    let recent_data = queue.get_data_in_range(past_30_seconds, current_time).await;
    println!("\n📋 Retrieved {} data points from last 30 seconds", recent_data.len());
    
    Ok(())
}

/// Demonstrates various queue configurations
async fn demonstrate_configurations() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔧 Demonstrating Queue Configurations");
    println!("{}", "=".repeat(40));

    // Configuration 1: High-frequency monitoring
    let high_freq_config = QueueConfig {
        queue_type: QueueType::Circular {
            capacity: 500,
            base_interval: 100, // 100ms
        },
        mode: Mode::default(),
        model: Model::default(),
        hook: std::sync::Arc::new(|_: Vec<SensorReading>| {
            Ok(generate_sensor_data("high_freq"))
        }),
    };

    // Configuration 2: Low-frequency, high-capacity storage
    let archival_config = QueueConfig {
        queue_type: QueueType::Circular {
            capacity: 10000,
            base_interval: 60000, // 1 minute
        },
        mode: Mode::default(),
        model: Model::default(),
        hook: std::sync::Arc::new(|_: Vec<SensorReading>| {
            Ok(generate_sensor_data("archival"))
        }),
    };

    // Configuration 3: Real-time processing
    let realtime_config = QueueConfig {
        queue_type: QueueType::Circular {
            capacity: 50,
            base_interval: 10, // 10ms
        },
        mode: Mode::default(),
        model: Model::default(),
        hook: std::sync::Arc::new(|_: Vec<SensorReading>| {
            Ok(generate_sensor_data("realtime"))
        }),
    };

    println!("1️⃣  High-frequency monitoring: 100ms intervals, 500 capacity");
    println!("2️⃣  Archival storage: 1-minute intervals, 10k capacity");
    println!("3️⃣  Real-time processing: 10ms intervals, 50 capacity");
    
    // Test each configuration briefly
    for (name, config) in [("High-freq", high_freq_config), ("Archival", archival_config), ("Real-time", realtime_config)] {
        let mut queue = Queue::new(config);
        let _handle = queue.start_server()?;
        
        sleep(Duration::from_millis(100)).await;
        
        let stats = queue.get_stats();
        println!("{}: interval={}ms, size={}, capacity={}", 
                 name, stats.interval_ms, stats.size, stats.capacity);
    }

    Ok(())
}

/// Demonstrates error handling and edge cases
async fn demonstrate_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⚠️  Demonstrating Error Handling");
    println!("{}", "=".repeat(35));

    let queue_config = QueueConfig {
        queue_type: QueueType::Circular {
            capacity: 10,
            base_interval: 1000,
        },
        mode: Mode::default(),
        model: Model::default(),
        hook: std::sync::Arc::new(|history: Vec<SensorReading>| {
            if history.len() > 5 {
                Err(QueueError::Other("Too many historical items".to_string()))
            } else {
                Ok(generate_sensor_data("error_test"))
            }
        }),
    };

    let mut queue = Queue::new(queue_config);

    // Test successful operations
    for i in 0..3 {
        let reading = generate_sensor_data(&format!("sensor_{}", i));
        match queue.publish(reading).await {
            Ok(_) => println!("✅ Successfully published reading {}", i),
            Err(e) => println!("❌ Failed to publish reading {}: {}", i, e),
        }
    }

    // Test queue operations
    match queue.get_latest().await {
        Some(data) => println!("✅ Latest data retrieved: sensor_id={}", data.1.sensor_id),
        None => println!("⚠️  No data available in queue"),
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Autoqueues IoT Sensor Data Processing Examples");
    println!("This example demonstrates real-world IoT applications\n");

    // Run the main IoT system demonstration
    run_iot_system().await?;

    // Demonstrate various configurations
    demonstrate_configurations().await?;

    // Demonstrate error handling
    demonstrate_error_handling().await?;

    println!("\n🎉 All IoT examples completed successfully!");
    println!("\nKey takeaways:");
    println!("- AIMD provides adaptive interval management (now constant interval)");
    println!("- Multiple queue configurations suit different use cases");
    println!("- Robust error handling for production environments");
    println!("- Easy integration with existing IoT systems");

    Ok(())
}