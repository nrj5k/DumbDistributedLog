//! Integration tests for AutoQueues - addressing risk factors
//!
//! These tests verify end-to-end functionality, type safety, and real-world scenarios.

use autoqueues::*;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_end_to_end_expression_with_queue() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing End-to-End Expression + Queue Integration");
    
    // Create a queue with expression monitoring
    let expression = ExpressionF64::new("(local.cpu + local.memory) / 2.0")?;
    let mut queue: SimpleQueue<f64> = SimpleQueue::with_expression(Box::new(expression.clone()));
    
    // Verify queue has expression
    assert!(queue.has_expression());
    assert!(queue.get_expression().is_some());
    
    // Publish some data
    queue.publish(75.0)?;
    queue.publish(85.0)?;
    queue.publish(65.0)?;
    
    // Get latest data
    let latest = queue.get_latest();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().1, 65.0);
    
    // Test expression evaluation with real data
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu".to_string(), 75.0);
    local_vars.insert("memory".to_string(), 85.0);
    let global_vars = HashMap::new();
    
    let result = expression.evaluate(&local_vars, &global_vars)?;
    assert_eq!(result, 80.0); // (75 + 85) / 2 = 80
    
    println!("✅ End-to-end expression + queue integration test passed");
    Ok(())
}

#[test]
fn test_type_safety_across_components() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 Testing Type Safety Across Components");
    
    // Test different data types with queues
    let mut int_queue: SimpleQueue<i32> = SimpleQueue::new();
    let mut string_queue: SimpleQueue<String> = SimpleQueue::new();
    let mut struct_queue: SimpleQueue<TestData> = SimpleQueue::new();
    
    // Test integer queue
    int_queue.publish(42)?;
    int_queue.publish(123)?;
    assert_eq!(int_queue.get_size(), 2);
    
    // Test string queue
    string_queue.publish("hello".to_string())?;
    string_queue.publish("world".to_string())?;
    assert_eq!(string_queue.get_size(), 2);
    
    // Test struct queue
    let data1 = TestData { id: 1, value: 3.14 };
    let data2 = TestData { id: 2, value: 2.71 };
    struct_queue.publish(data1.clone())?;
    struct_queue.publish(data2.clone())?;
    assert_eq!(struct_queue.get_size(), 2);
    
    // Verify type-specific operations
    let int_latest = int_queue.get_latest();
    assert!(int_latest.is_some());
    assert_eq!(int_latest.unwrap().1, 123);
    
    let string_latest = string_queue.get_latest();
    assert!(string_latest.is_some());
    assert_eq!(string_latest.unwrap().1, "world");
    
    let struct_latest = struct_queue.get_latest();
    assert!(struct_latest.is_some());
    assert_eq!(struct_latest.unwrap().1.value, 2.71);
    
    println!("✅ Type safety test passed");
    Ok(())
}

#[test]
fn test_real_system_metrics_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Testing Real System Metrics Integration");
    
    let mut metrics = MetricsCollector::new();
    
    // Get actual system metrics
    let system_metrics = metrics.get_all_metrics();
    
    // Verify we got real data
    assert!(system_metrics.cpu_usage_percent >= 0.0 && system_metrics.cpu_usage_percent <= 100.0);
    assert!(system_metrics.memory_usage.usage_percent >= 0.0 && system_metrics.memory_usage.usage_percent <= 100.0);
    assert!(system_metrics.disk_usage.usage_percent >= 0.0 && system_metrics.disk_usage.usage_percent <= 100.0);
    
    // Test health score calculation
    let health_score = metrics.get_health_score();
    assert!(health_score <= 100);
    
    // Test expression evaluation with real metrics
    let expression = ExpressionF64::new("local.cpu_usage + local.memory_usage")?;
    
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu_usage".to_string(), system_metrics.cpu_usage_percent as f64);
    local_vars.insert("memory_usage".to_string(), system_metrics.memory_usage.usage_percent as f64);
    let global_vars = HashMap::new();
    
    let combined_usage = expression.evaluate(&local_vars, &global_vars)?;
    assert!(combined_usage > 0.0);
    
    println!("✅ Real system metrics integration test passed");
    println!("   CPU: {:.1}%, Memory: {:.1}%, Combined: {:.1}", 
             system_metrics.cpu_usage_percent, 
             system_metrics.memory_usage.usage_percent, 
             combined_usage);
    
    Ok(())
}

#[test]
fn test_error_handling_robustness() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️  Testing Error Handling Robustness");
    
    // Test empty expression
    let empty_result = ExpressionF64::new("");
    assert!(empty_result.is_err());
    
    // Test invalid expression syntax - this will fail during validation (improved behavior)
    let invalid_result = ExpressionF64::new("local.cpu +++ invalid");
    assert!(invalid_result.is_err());
    
    // Test an expression that passes validation but fails during evaluation
    let eval_fail_expr = ExpressionF64::new("local.undefined_var")?;
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu".to_string(), 75.0); // Don't include "undefined_var"
    let global_vars = HashMap::new();
    
    let invalid_eval_result = eval_fail_expr.evaluate(&local_vars, &global_vars);
    assert!(invalid_eval_result.is_err());
    
    // Test division by zero in runtime
    let div_expr = ExpressionF64::new("local.value / local.zero")?;
    let mut local_vars = HashMap::new();
    local_vars.insert("value".to_string(), 100.0);
    local_vars.insert("zero".to_string(), 0.0);
    let global_vars = HashMap::new();
    
    let div_result = div_expr.evaluate(&local_vars, &global_vars);
    assert!(div_result.is_err()); // Should handle division by zero
    
    // Test undefined variables
    let undef_expr = ExpressionF64::new("local.missing")?;
    let empty_local = HashMap::new();
    let empty_global = HashMap::new();
    
    let undef_result = undef_expr.evaluate(&empty_local, &empty_global);
    assert!(undef_result.is_err());
    
    // Test queue errors
    let queue: SimpleQueue<i32> = SimpleQueue::new();
    
    // Queue operations should handle mutex errors gracefully
    let size = queue.get_size();
    assert_eq!(size, 0); // Should return 0 even if mutex is locked
    
    println!("✅ Error handling robustness test passed");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_queue_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Testing Concurrent Queue Operations");
    
    let queue = Arc::new(tokio::sync::Mutex::new(SimpleQueue::<i32>::new()));
    let mut handles = vec![];
    
    // Spawn multiple concurrent publishers
    for i in 0..10 {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            let mut q = queue_clone.lock().await;
            for j in 0..5 {
                let value = i * 100 + j;
                q.publish(value).expect("Failed to publish");
            }
        });
        handles.push(handle);
    }
    
    // Wait for all publishers to complete
    for handle in handles {
        handle.await?;
    }
    
    // Verify all data was published
    let final_queue = queue.lock().await;
    assert_eq!(final_queue.get_size(), 50); // 10 publishers * 5 values each
    
    // Get latest values and verify they're in reasonable range
    let latest_values = final_queue.get_latest_n(10);
    assert!(!latest_values.is_empty());
    
    for value in &latest_values {
        assert!(*value >= 0 && *value < 1000);
    }
    
    println!("✅ Concurrent operations test passed");
    Ok(())
}

#[test]
fn test_configuration_system_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙️  Testing Configuration System Integration");
    
    // Test standard configuration
    let standard_config = QueueConfig::create_standard();
    assert!(!standard_config.get_base_metrics().is_empty());
    
    // Test minimal configuration
    let minimal_config = QueueConfig::create_minimal();
    assert!(!minimal_config.get_base_metrics().is_empty());
    
    // Test TOML parsing
    let toml_content = r#"
[queues]
base = ["cpu_usage", "memory_usage"]

[queues.derived.combined_load]
formula = "(local.cpu_usage + local.memory_usage) / 2.0"
"#;
    
    let custom_config = QueueConfig::from_str(toml_content)?;
    assert_eq!(custom_config.get_base_metrics(), &["cpu_usage", "memory_usage"]);
    
    let derived_formulas: Vec<_> = custom_config.get_derived_formulas().collect();
    assert_eq!(derived_formulas.len(), 1);
    assert_eq!(derived_formulas[0].0, "combined_load");
    
    // Test expression evaluation with config-derived formula
    let expression = ExpressionF64::new(derived_formulas[0].1)?;
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu_usage".to_string(), 75.0);
    local_vars.insert("memory_usage".to_string(), 85.0);
    let global_vars = HashMap::new();
    
    let result = expression.evaluate(&local_vars, &global_vars)?;
    assert_eq!(result, 80.0);
    
    println!("✅ Configuration system integration test passed");
    Ok(())
}

// Helper struct for type safety tests
#[derive(Debug, Clone, PartialEq)]
struct TestData {
    id: i32,
    value: f64,
}
