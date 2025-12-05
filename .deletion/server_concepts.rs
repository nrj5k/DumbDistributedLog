//! # Queue Server Concept Example
//!
//! This example demonstrates the autonomous server capabilities of autoqueues.
//! The server runs independently, automatically processing data based on configured intervals.

use autoqueues::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Server demonstrations focused on queue server concepts
fn process_metrics(data: Vec<i32>) -> Result<i32, QueueError> {
    if data.is_empty() {
        return Ok(50);
    }
    let avg = data.iter().sum::<i32>() / data.len() as i32;
    Ok(avg)
}

/// Demonstrates autonomous server
async fn demonstrate_autonomous_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Queue Server Concept Demonstration");
    println!("=========================================");

    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        100,  // 100ms base interval
        2,
        "metrics_server.log".to_string(),
        "system_metrics_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_metrics),
        Model::Linear,
        queue_type,
    );

    let mut work_queue = Queue::new(config.clone());
    let server = Queue::new(config).start_server()?;
    
    println!("✅ METRICS SERVER STARTED");
    println!("   Configured for 100ms automatic processing intervals");

    // Simulate injecting data while server runs in background
    let scenarios = vec![
        vec![60, 65, 70],    // Normal load
        vec![80, 85, 90],    // High load  
        vec![45, 50, 55],    // Low load
    ];

    for (idx, batch) in scenarios.iter().enumerate() {
        println!("   📦 Processing batch {}: {:?}", idx + 1, batch);
        
        for &value in batch {
            work_queue.publish(value).await?;
        }
        
        sleep(Duration::from_millis(400)).await;
        
        if let Some(latest) = work_queue.get_latest().await {
            println!("   📊 Latest processed: {} (timestamp: {})", latest.1, latest.0);
        }
    }

    println!("\n📈 SERVER STATISTICS:");
    let stats = work_queue.get_stats();
    println!("   Items processed: {}", stats.size);
    println!("   Current rate: {}ms intervals", stats.interval_ms);
    
    println!("\n🛑 Stopping server gracefully...");
    server.shutdown().await?;
    println!("   ✅ Server shutdown complete");

    Ok(())
}

/// Multi-server demonstration
async fn demonstrate_multi_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏢 Multi-Server Architecture Demo");
    println!("===================================");

    // Different servers with different speeds
    let server_configs = vec![
        ("Real-time Analytics", 50, 1),     // 50ms - ultra fast
        ("Standard Analytics", 500, 2),     // 500ms - standard
        ("Audit Logger", 2000, 5),          // 2000ms - slow audit frequency
    ];

    let mut server_handles = Vec::new();
    let mut work_queues = Vec::new();

    println!("\n🎯 Creating multiple servers with different intervals:");
    for (name, interval, factor) in server_configs.iter() {
        let queue_type = QueueType::new(
            QueueValue::NodeCapacity,
            *interval,
            *factor,
            format!("{}.log", name.to_lowercase().replace(" ", "_")),
            format!("{}_var", name.to_lowercase().replace(" ", "_")),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(process_metrics),
            Model::Linear,
            queue_type,
        );

        let work_queue = Queue::new(config.clone());
        let server = Queue::new(config).start_server()?;
        
        println!("   ✅ {}: {}ms intervals", name, interval);
        
        work_queues.push(work_queue);
        server_handles.push(server);
    }

    println!("\n🔄 All servers running simultaneously:");
    
    // Show real-time processing rates
    for i in 0..4 {
        work_queues[0].publish(85 + i).await?; // Real-time gets recent data
        work_queues[1].publish(70 - i).await?; // Standard processor
        work_queues[2].publish(50).await?;     // Audit logger gets events
        
        sleep(Duration::from_millis(200)).await;
        
        // Real-time server will process this quickly
        if let Some(latest) = work_queues[0].get_latest().await {
            println!("   💨 Real-time: {} ({}ms)", latest.1, latest.0);
        }
    }

    println!("\n📊 MULTI-SERVER PERFORMANCE:");
    for (idx, queue) in work_queues.iter().enumerate() {
        let stats = queue.get_stats();
        println!("   {}\t- {} items in {}ms intervals", 
                 server_configs[idx].0, stats.size, server_configs[idx].1);
    }

    println!("\n🛑 Shutting down all servers...");
    for (idx, server) in server_handles.into_iter().enumerate() {
        server.shutdown().await?;
        println!("   ✅ {} stopped", server_configs[idx].0);
    }

    Ok(())
}

/// Server restart and recovery
async fn demonstrate_server_recovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 Server Restart & Recovery Demo");
    println!("===================================");

    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        300,  // 300ms intervals
        3,
        "recovery_demo.log".to_string(),
        "recovery_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_metrics),
        Model::Linear,
        queue_type,
    );

    println!("\n🚀 Starting initial server...");
    let mut work_queue = Queue::new(config.clone());
    let first_server = Queue::new(config.clone()).start_server()?;
    
    println!("   📊 Injecting initial data: [70, 75, 80]");
    work_queue.publish(70).await?;
    work_queue.publish(75).await?;
    work_queue.publish(80).await?;
    
    first_server.shutdown().await?;
    println!("   ✅ First server shutdown complete");

    println!("\n🔄 Server recovery with restart...");
    sleep(Duration::from_secs(1)).await;
    
    let mut recovery_queue = Queue::new(config.clone());
    let recovery_server = Queue::new(config.clone()).start_server()?;
    
    println!("   🔄 New server started for recovery");
    let stats = recovery_queue.get_stats();
    
    println!("📈 Recovery statistics:");
    println!("   Current queue: {} items, {}ms intervals", stats.size, stats.interval_ms);
    
    println!("   📂 Adding recovery data: [85, 90, 95]");
    recovery_queue.publish(85).await?;
    recovery_queue.publish(90).await?;
    recovery_queue.publish(95).await?;
    
    if let Some(latest) = recovery_queue.get_latest().await {
        println!("   📊 Latest recovered value: {}", latest.1);
    }
    
    recovery_server.shutdown().await?;
    println!("   ✅ Recovery server stopped");
    
    Ok(())
}

/// Queue server concepts masterclass
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌟 QUEUE SERVER CONCEPTS MASTERCLASS");
    println!("{}", "=".repeat(50));
    println!("🎯 This comprehensive demo covers:");
    println!("  1. Single autonomous server (real-time processing)");
    println!("  2. Multi-server architecture (different speeds)");
    println!("  3. Server restart & recovery");
    println!("  4. Graceful shutdown handling");

    demonstrate_autonomous_server().await?;
    sleep(Duration::from_secs(1)).await;

    demonstrate_multi_server().await?;  
    sleep(Duration::from_secs(1)).await;

    demonstrate_server_recovery().await?;
    sleep(Duration::from_secs(1)).await;

    println!("\n{}", "=".repeat(50));
    println!("🎉 ALL SERVER DEMONSTRATIONS COMPLETED!");
    println!("\n💡 KEY TAKEAWAYS:");
    println!("  • Servers run independently with configured intervals");
    println!("  • Multiple servers can coexist and process different data types");
    println!("  • Servers handle graceful shutdown and data persistence");
    println!("  • Real-time statistics show processing status");
    println!("  • Server design enables scalable architecture patterns");

    Ok(())
}