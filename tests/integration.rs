//! Simple integration tests for the autoqueues library using current API

use autoqueues::*;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_basic_queue_operations() -> Result<(), Box<dyn std::error::Error>> {
    // Create a basic queue using SimpleQueue
    let mut queue = SimpleQueue::new();

    // Test initial state
    assert_eq!(queue.get_size(), 0);
    assert_eq!(queue.get_latest(), None);
    assert_eq!(queue.get_latest_n(5), Vec::<i32>::new());

    // Add some data
    queue.publish(100)?;
    queue.publish(200)?;

    // Verify data was added
    assert_eq!(queue.get_size(), 2);

    // Get latest data
    let latest = queue.get_latest();
    assert!(latest.is_some());
    let (_timestamp, value) = latest.unwrap();
    assert_eq!(value, 200);

    // Get latest N values
    let recent_values = queue.get_latest_n(2);
    assert_eq!(recent_values, vec![200, 100]);

    Ok(())
}

#[tokio::test]
async fn test_queue_server() -> Result<(), Box<dyn std::error::Error>> {
    // Create queues for testing
    let queue_for_server = SimpleQueue::<i32>::new();
    let mut queue_for_data = queue_for_server.clone();

    // Start server (this will run in background)
    let handle = queue_for_server.start_server()?;

    // Add some data to the queue
    queue_for_data.publish(42)?;
    queue_for_data.publish(84)?;

    // Let server run
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify data was processed
    let stats = queue_for_data.get_size();
    assert_eq!(stats, 2);

    let latest = queue_for_data.get_latest();
    assert_eq!(latest.map(|(_, v)| v), Some(84));

    // Shutdown server
    handle.shutdown().await?;

    Ok(())
}

#[tokio::test]
async fn test_expression_integration() -> Result<(), Box<dyn std::error::Error>> {
    // Create an expression for computing health scores - using simpler expression
    let expression = ExpressionF64::new("local.cpu + local.memory")?;

    // Create a queue with expression
    let expr_box = Box::new(expression.clone());
    let queue = SimpleQueue::with_expression(expr_box);

    // Verify expression is set
    assert!(queue.has_expression());
    assert!(queue.get_expression().is_some());

    // Check required variables - they should include the variable names with local. prefix
    let required_vars = expression.required_local_vars();
    println!("Required vars: {:?}", required_vars);
    assert!(required_vars.contains(&String::from("cpu")));
    assert!(required_vars.contains(&String::from("memory")));
    assert!(expression.required_global_vars().is_empty());

    // Test expression evaluation with sample data
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu".to_string(), 50.0);
    local_vars.insert("memory".to_string(), 70.0);

    let result = expression.evaluate(&local_vars, &HashMap::new())?;
    assert_eq!(result, 120.0);

    Ok(())
}

#[tokio::test]
async fn test_queue_capacity() -> Result<(), Box<dyn std::error::Error>> {
    let mut queue = SimpleQueue::<i32>::new();

    // Fill with many data points
    for i in 0..5000 {
        queue.publish(i)?;
    }

    // Basic queue operations should still work
    assert_eq!(queue.get_size(), 5000);

    // Latest should be most recent
    let latest = queue.get_latest();
    assert!(latest.is_some());
    let (_timestamp, value) = latest.unwrap();
    assert_eq!(value, 4999);

    // Get multiple recent values
    let recent = queue.get_latest_n(5);
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0], 4999);
    assert_eq!(recent[4], 4995);

    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    // Test queue operations with errors
    let mut queue = SimpleQueue::<i32>::new();

    // Basic operations should not fail for a standard queue
    queue.publish(42)?;
    queue.publish(84)?;

    // Get operations should also not fail
    let latest = queue.get_latest();
    assert_eq!(latest.map(|(_, v)| v), Some(84));

    let _recent = queue.get_latest_n(3);

    // Test expression errors
    let undefined_var = ExpressionF64::new("1 + local.undefined_var").unwrap();
    let result = undefined_var.evaluate(&HashMap::new(), &HashMap::new());
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_queue_config_integration() -> Result<(), Box<dyn std::error::Error>> {
    // Test using standard configuration
    let config = QueueConfig::create_standard();
    
    // Verify base metrics
    let base_metrics = config.get_base_metrics();
    assert!(!base_metrics.is_empty());
    assert!(base_metrics.contains(&String::from("cpu_percent")));
    assert!(base_metrics.contains(&String::from("memory_percent")));
    assert!(base_metrics.contains(&String::from("drive_percent")));

    // Test derived formulas
    let derived_formulas: Vec<_> = config.get_derived_formulas().collect();
    assert!(!derived_formulas.is_empty());
    
    // Check specific derived formulas exist
    assert!(config.get_derived_formula("health_score").is_some());
    assert!(config.get_derived_formula("weighted_health").is_some());

    // Test expression from derived formula
    let health_expression = ExpressionF64::new(config.get_derived_formula("health_score").unwrap())?;
    
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu_percent".to_string(), 40.0);
    local_vars.insert("memory_percent".to_string(), 50.0);
    local_vars.insert("drive_percent".to_string(), 60.0);

    let health_score = health_expression.evaluate(&local_vars, &HashMap::new())?;
    assert_eq!(health_score, 50.0); // (40 + 50 + 60) / 3

    Ok(())
}

#[tokio::test]
async fn test_queue_with_toml_config() -> Result<(), Box<dyn std::error::Error>> {
    // Create a TOML configuration string
    let toml_config = r#"
[queues]
base = ["cpu_usage", "memory_usage", "network_throughput"]

[queues.derived.network_efficiency]
formula = "(local.network_throughput / (local.cpu_usage + local.memory_usage)) * 100"

[queues.derived.system_load_average]
formula = "(local.cpu_usage + local.memory_usage) / 2"
"#;

    // Parse configuration from TOML string
    let config = QueueConfig::from_str(toml_config)?;
    
    // Verify base metrics
    let base_metrics = config.get_base_metrics();
    assert_eq!(base_metrics.len(), 3);
    assert!(base_metrics.contains(&String::from("cpu_usage")));
    assert!(base_metrics.contains(&String::from("memory_usage")));
    assert!(base_metrics.contains(&String::from("network_throughput")));

    // Verify derived formulas
    assert!(config.get_derived_formula("network_efficiency").is_some());
    assert!(config.get_derived_formula("system_load_average").is_some());

    // Test system load average
    let load_avg_expr = ExpressionF64::new(config.get_derived_formula("system_load_average").unwrap())?;
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu_usage".to_string(), 50.0);
    local_vars.insert("memory_usage".to_string(), 30.0);
    local_vars.insert("network_throughput".to_string(), 8000.0);
    let load_avg = load_avg_expr.evaluate(&local_vars, &HashMap::new())?;
    assert_eq!(load_avg, 40.0); // (50 + 30) / 2

    Ok(())
}

#[tokio::test]
async fn test_queue_string_data() -> Result<(), Box<dyn std::error::Error>> {
    // Test queue with string data
    let mut queue = SimpleQueue::<String>::new();

    queue.publish("test string 1".to_string())?;
    queue.publish("test string 2".to_string())?;
    queue.publish("test string 3".to_string())?;

    assert_eq!(queue.get_size(), 3);

    let latest = queue.get_latest();
    assert_eq!(latest.map(|(_, v)| v), Some("test string 3".to_string()));

    let recent = queue.get_latest_n(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0], "test string 3");
    assert_eq!(recent[1], "test string 2");

    Ok(())
}

#[tokio::test]
async fn test_queue_float_data() -> Result<(), Box<dyn std::error::Error>> {
    // Test queue with floating point data
    let mut queue = SimpleQueue::<f64>::new();

    queue.publish(1.5)?;
    queue.publish(2.7)?;
    queue.publish(3.14159)?;

    assert_eq!(queue.get_size(), 3);

    let latest = queue.get_latest();
    assert_eq!(latest.map(|(_, v)| v), Some(3.14159));

    let recent = queue.get_latest_n(4); // More than available
    assert_eq!(recent.len(), 3);

    Ok(())
}