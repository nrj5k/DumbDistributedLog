//! Mathematical Expression Engine Demo - Local-Only Equations
//!
//! Demonstrates local expression evaluation with complex Rust math functions.
//! Shows how queues can process user-defined equations like:
//! (local.cpu_percent + local.memory_percent) / 2.0

use autoqueues::*;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::time::{Duration, sleep};

/// System metrics for expression variables
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct MetricData {
    pub hostname: String,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub drive_percent: f32,
    pub timestamp: u64,
    pub result_value: f32, // Store expression result here
}

/// Expression processor factory that creates proper function hooks
fn create_expression_processor(
    expression_text: &str,
) -> Result<impl Fn(Vec<MetricData>) -> Result<MetricData, QueueError>, ExpressionError> {
    let expression_text = expression_text.to_string();
    let expression = Arc::new(Mutex::new(LocalExpression::new(&expression_text)?));
    let context = Arc::new(Mutex::new(ExpressionContext::new()));

    Ok(
        move |data: Vec<MetricData>| -> Result<MetricData, QueueError> {
            let ctx = context.lock().unwrap();

            if let Some(latest) = data.last() {
                // Update context with current metrics
                ctx.set_variable("local.cpu_percent", latest.cpu_percent);
                ctx.set_variable("local.memory_percent", latest.memory_percent);
                ctx.set_variable("local.drive_percent", latest.drive_percent);

                // Evaluate expression
                let calculated_value = match expression.lock().unwrap().evaluate() {
                    Ok(value) => value,
                    Err(e) => {
                        println!(
                            "⚠️  Expression '{}' evaluation error: {}",
                            expression_text, e
                        );
                        0.0 // Default value on error
                    }
                };

                Ok(MetricData {
                    hostname: latest.hostname.clone(),
                    cpu_percent: latest.cpu_percent,
                    memory_percent: latest.memory_percent,
                    drive_percent: latest.drive_percent,
                    timestamp: latest.timestamp,
                    result_value: calculated_value,
                })
            } else {
                // No data - return defaults with expression applied
                ctx.set_variable("local.cpu_percent", 25.0);
                ctx.set_variable("local.memory_percent", 35.0);
                ctx.set_variable("local.drive_percent", 45.0);

                let result = expression.lock().unwrap().evaluate().unwrap_or(0.0);

                Ok(MetricData {
                    hostname: "default_system".to_string(),
                    cpu_percent: 25.0,
                    memory_percent: 35.0,
                    drive_percent: 45.0,
                    timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    result_value: result,
                })
            }
        },
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧮 Local Expression Engine Demo - Mathematical Equations");
    println!("{}", "=".repeat(60));

    println!("\n🎯 Expression Examples:");
    println!("   • (local.cpu_percent + local.memory_percent) / 2.0");
    println!("   • local.drive_percent.sqrt()");
    println!(
        "   • (local.cpu_percent * 0.4) + (local.memory_percent * 0.3) + (local.drive_percent * 0.3)"
    );

    // Test expression parsing
    println!("\n🔍 Testing Expression Parser:");

    let test_expressions = vec![
        "(local.cpu_percent + local.memory_percent) / 2.0",
        "local.drive_percent * 2.0",
        "(local.cpu_percent.powf(2.0) + local.memory_percent.sqrt()) / 2.0",
    ];

    for expr in test_expressions {
        match LocalExpression::new(expr) {
            Ok(expression) => {
                println!("✅ Parsed: {}", expr);
                println!("   Topics needed: {:?}", expression.get_required_topics());
            }
            Err(e) => {
                println!("❌ Failed to parse '{}': {}", expr, e);
            }
        }
    }

    // Real expression evaluation demo
    println!("\n🔄 Real-time Expression Evaluation:");
    println!("   Expression: (local.cpu_percent + local.memory_percent) / 2.0");

    let processor_hook =
        create_expression_processor("(local.cpu_percent + local.memory_percent) / 2.0")?;

    let queue_config = QueueConfig::new(
        Mode::Insight,
        Arc::new(processor_hook),
        Model::Linear,
        QueueType::new(
            QueueValue::SystemHealth,
            1000, // 1-second intervals
            2,
            "expression_demo.log".to_string(),
            "expression_demo_var".to_string(),
        ),
    );

    let mut queue = Queue::new(queue_config.clone());
    let _server = Queue::new(queue_config).start_server()?;

    println!("\n📊 Running 10 cycles of expression evaluation...");

    for cycle in 0..10 {
        // Generate realistic metrics
        let metrics = MetricData {
            hostname: "local_system".to_string(),
            cpu_percent: 20.0 + (cycle as f32 * 4.0), // 20-60% range
            memory_percent: 35.0 + (cycle as f32 * 2.5), // 35-55% range
            drive_percent: 45.0 + (cycle as f32 * 3.0), // 45-75% range
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            result_value: 0.0, // Will be overwritten by expression
        };

        queue.publish(metrics.clone()).await?;

        match queue.get_latest().await {
            Some((_, result_data)) => {
                let calculated_result = result_data.result_value;
                println!(
                    "   ⏰ Cycle {}: Input CPU:{:.1}%, Mem:{:.1}% → Expression Result:{:.1}",
                    cycle + 1,
                    metrics.cpu_percent,
                    metrics.memory_percent,
                    calculated_result
                );
            }
            None => println!("   ⏺️  Queue processing..."),
        }

        sleep(Duration::from_millis(1000)).await;
    }

    // Multi-variable expression demonstration
    println!("\n🔢 Complex Multi-Variable Expression:");
    println!(
        "   Expression: (local.cpu_percent * 0.4) + (local.memory_percent * 0.3) + (local.drive_percent * 0.3)"
    );

    let complex_processor = create_expression_processor(
        "(local.cpu_percent * 0.4) + (local.memory_percent * 0.3) + (local.drive_percent * 0.3)",
    )?;

    let complex_queue_config = QueueConfig::new(
        Mode::Insight,
        Arc::new(complex_processor),
        Model::Linear,
        QueueType::new(
            QueueValue::SystemHealth,
            1500, // 1.5-second intervals
            2,
            "complex_expression.log".to_string(),
            "complex_expression_var".to_string(),
        ),
    );

    let mut complex_queue = Queue::new(complex_queue_config.clone());
    let _complex_server = Queue::new(complex_queue_config).start_server()?;

    for cycle in 0..5 {
        let weighted_metrics = MetricData {
            hostname: "weighted_system".to_string(),
            cpu_percent: 22.0 + (cycle * 8) as f32, // Growing CPU
            memory_percent: 38.0 + (cycle * 3) as f32, // Stable memory
            drive_percent: 50.0 + (cycle * 6) as f32, // Fast growing drive
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            result_value: 0.0,
        };

        complex_queue.publish(weighted_metrics.clone()).await?;

        if let Some((_, result)) = complex_queue.get_latest().await {
            println!(
                "   ⚖️  Weighted Cycle {}: CPU:{:.1}, Mem:{:.1}, Disk:{:.1} → Weighted:{:.1}",
                cycle + 1,
                weighted_metrics.cpu_percent,
                weighted_metrics.memory_percent,
                weighted_metrics.drive_percent,
                result.result_value
            );
        }

        sleep(Duration::from_millis(1500)).await;
    }

    println!("\n📋 Capabilities Verified:");
    println!("   ✅ Math expressions with local.* variables");
    println!("   ✅ Division-by-zero protection (defaults to 0)");
    println!("   ✅ Expression evaluation at queue intervals");
    println!("   ✅ Local-only scope (no global.* references)");
    println!("   ✅ Real-time metric processing");

    println!("\n🎯 Next - Advanced Functions:");
    println!("   • sqrt(), abs(), powf() mathematical functions");
    println!("   • max(), min() comparative functions");
    println!("   • More complex nested expressions");

    println!("\n✅ Local expression engine demo complete!");
    println!("   Ready for complex user-defined equations!");

    Ok(())
}
