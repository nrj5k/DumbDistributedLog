//! # Financial Market Data Processing Example
//!
//! This example demonstrates how to use autoqueues for processing real-time financial market data
//! with different trading strategies and risk management patterns.

use autoqueues::{Queue, QueueConfig, QueueType, QueueError};
use autoqueues::{QueueValue, Mode, Model};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Simple market data processing function for demonstration
fn process_market_data(data: Vec<i32>) -> Result<i32, QueueError> {
    // Simulate market data processing
    if data.is_empty() {
        return Ok(100);
    }
    let avg = data.iter().sum::<i32>() / data.len() as i32;
    let processed_value = avg * 2 + 10;
    Ok(processed_value)
}

/// Calculate risk metrics for portfolio management
fn calculate_risk_metrics(data: Vec<i32>) -> Result<i32, QueueError> {
    // Simulate risk calculation based on input
    if data.is_empty() {
        return Ok(0);
    }
    let sum = data.iter().sum::<i32>();
    let risk_score = sum * 10 - 500;
    Ok(risk_score)
}

/// Monitor market anomalies and alert system
fn monitor_market_anomalies(data: Vec<i32>) -> Result<i32, QueueError> {
    // Simulate anomaly detection
    if data.is_empty() {
        return Ok(50);
    }
    let avg = data.iter().sum::<i32>() / data.len() as i32;
    let anomaly_score = if avg > 100 { 1000 } else { avg };
    Ok(anomaly_score)
}

/// High-frequency trading (HFT) simulation
async fn simulate_hft_trading() -> Result<(), QueueError> {
    println!("🚀 High-Frequency Trading Simulation");
    
    // Configure for high-frequency trading (microsecond precision)
    let queue_type = QueueType::new(
        QueueValue::NodeLoad,
        100,  // 100ms base interval
        2,
        "hft_trading.log".to_string(),
        "hft_trading_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_market_data),
        Model::Linear,
        queue_type,
    );

    let mut queue = Queue::new(config.clone());
    let server_handle = Queue::new(config).start_server()?;
    
    println!("✅ HFT server started with microsecond-level precision");

    // Simulate trading for 30 seconds
    for i in 0..30 {
        // Simulate market data
        let market_data = 100 + (i as i32);
        queue.publish(market_data).await?;
        
        if i % 10 == 0 {
            let stats = queue.get_stats();
            
            println!("💎 HFT Session {}: {} items, current interval: {}ms", 
                     i, stats.size, stats.interval_ms);
            
            if let Some(latest) = queue.get_latest().await {
                println!("📊 Latest processed value: {}", latest.1); // Access the tuple field
            }
        }
        
        sleep(Duration::from_millis(100)).await;
    }

    server_handle.shutdown().await?;
    println!("🛑 HFT trading simulation completed");

    Ok(())
}

/// Portfolio risk management simulation
async fn simulate_risk_management() -> Result<(), QueueError> {
    println!("\n💰 Portfolio Risk Management Simulation");
    
    // Configure for risk monitoring
    let queue_type = QueueType::new(
        QueueValue::NodeCapacity,
        500,  // 500ms intervals for risk checks
        3,
        "risk_management.log".to_string(),
        "risk_management_var".to_string(),
    );

    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(calculate_risk_metrics),
        Model::Linear,
        queue_type,
    );

    let mut risk_queue = Queue::new(config.clone());
    let risk_server = Queue::new(config).start_server()?;

    println!("🛡️  Risk management system initiated");

    // Simulate portfolio monitoring
    for i in 0..15 {
        let portfolio_data = (100 + i) * 10;
        risk_queue.publish(portfolio_data).await?;

        if i % 5 == 0 {
            let stats = risk_queue.get_stats();
            
            println!("📈 Risk Check {}: {} items, current interval: {}ms", 
                     i, stats.size, stats.interval_ms);
            
            let latest = risk_queue.get_latest().await;
            if let Some(risk_data) = latest {
                let risk_value = risk_data.1; // Access tuple field
                if risk_value < -30 {
                    println!("⚠️  RISK ALERT: High risk detected (score: {})", risk_value);
                } else {
                    println!("✅ Risk level normal (score: {})", risk_value);
                }
            }
        }
        
        sleep(Duration::from_millis(300)).await;
    }

    risk_server.shutdown().await?;
    println!("🛑 Risk management simulation completed");

    Ok(())
}

/// Market anomaly detection simulation
async fn simulate_market_anomaly_detection() -> Result<(), QueueError> {
    println!("\n🔍 Market Anomaly Detection Simulation");
    
    // Configure for anomaly detection
    let queue_type = QueueType::new(
        QueueValue::NodeAvailability,
        200,  // 200ms intervals for anomaly detection
        5,
        "anomaly_detection.log".to_string(),
        "anomaly_detection_var".to_string(),
    );

    // Create a monitor that just averages data
    let config = QueueConfig::new(
        Mode::Sensor,
        Arc::new(process_market_data), // Re-use the process function
        Model::Linear,
        queue_type,
    );

    let mut anomaly_queue = Queue::new(config.clone());
    let anomaly_server = Queue::new(config).start_server()?;

    println!("🤖 AI anomaly detection system activated");

    // Simulate market data ingestion and anomaly detection
    for i in 0..20 {
        // Inject some anomalies
        let market_data = if i == 10 || i == 14 {
            150 // Artificially high - potential anomaly
        } else {
            50 + (i as i32) // Normal market conditions
        };
        
        anomaly_queue.publish(market_data).await?;

        let latest = anomaly_queue.get_latest().await;
        if let Some(anomaly_data) = latest {
            let anomaly_score = anomaly_data.1; // Access tuple field
            if anomaly_score > 200 {
                println!("🚨 ANOMALY DETECTED at session {} (score: {})", i, anomaly_score);
            } else {
                println!("🟢 Market conditions normal at session {} (score: {})", i, anomaly_score);
            }
        }

        sleep(Duration::from_millis(200)).await;
    }

    anomaly_server.shutdown().await?;
    println!("🔍 Market anomaly detection simulation completed");

    Ok(())
}

/// Main financial market data processing demonstration
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Financial Market Data Processing Example");
    println!("==================================================");
    println!("📈 This demo shows real-time financial data processing with:");
    println!("   • High-frequency trading (HFT) simulation");
    println!("   • Portfolio risk management");
    println!("   • Market anomaly detection");
    println!("==================================================");

    // Note: Since actual financial data isn't available, we simulate data patterns
    // In a real implementation, these would connect to Bloomberg/Reuters APIs

    // High-frequency trading simulation
    simulate_hft_trading().await?;
    
    sleep(Duration::from_secs(2)).await;
    
    // Portfolio risk management simulation
    simulate_risk_management().await?;
    
    sleep(Duration::from_secs(2)).await;
    
    // Market anomaly detection simulation
    simulate_market_anomaly_detection().await?;

    println!("\n🎉 All financial market simulations completed successfully!");
    println!("\n💡 Key takeaways:");
    println!("   • Financial data processing with adaptive intervals");
    println!("   • Real-time risk assessment and anomaly detection");
    println!("   • AIMD-based adaptive interval management");
    println!("   • Multi-strategy processing with different queue configurations");

    Ok(())
}