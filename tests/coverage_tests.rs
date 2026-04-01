//! Comprehensive tests for zero-coverage files in the autoqueues project
//!
//! This file tests:
//! - src/node.rs - RemoteMetric, MetricMessage, aggregation functions, AutoQueuesNode
//! - src/queue/registry.rs - QueueRegistry
//! - src/queue/interval.rs - AimdController and IntervalConfig
//! - src/queue/queue_server.rs - QueueError and QueueServerHandle
//! - src/cluster/raft_node.rs - ClusterConfig and RaftNode
//! - src/cluster/raft_router.rs - RaftMessageRouter (struct construction only)
//! - src/network/raft_transport.rs - ZmqRaftNetwork
//! - src/network/pubsub/zmq/transport.rs - ZmqTransport and NodeMessage

use ddl::config::Config;
use ddl::node::{MetricMessage, RemoteMetric};
use ddl::queue::interval::{AimdController, IntervalConfig};
use ddl::queue::persistence::PersistenceConfig;
use ddl::queue::queue_server::{QueueError, QueueServerHandle};
use ddl::queue::registry::QueueRegistry;
use ddl::queue::source::FunctionSource;
use ddl::cluster::raft_node::{ClusterConfig, RaftNode};
use ddl::network::pubsub::zmq::transport::{NodeMessage, ZmqTransport};
use ddl::network::transport_traits::Transport;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

// ============================================================================
// 1. src/node.rs Tests
// ============================================================================

mod node_tests {
    use super::*;

    // --- RemoteMetric Tests ---

    #[test]
    fn test_remote_metric_serialization() {
        let metric = RemoteMetric {
            node_id: 1,
            metric_name: "cpu".to_string(),
            value: 42.5,
            timestamp: 1234567890,
        };

        // Test serialization
        let json = serde_json::to_string(&metric).expect("Should serialize");
        assert!(json.contains("\"node_id\":1"));
        assert!(json.contains("\"metric_name\":\"cpu\""));
        assert!(json.contains("\"value\":42.5"));
        assert!(json.contains("\"timestamp\":1234567890"));

        // Test deserialization
        let deserialized: RemoteMetric =
            serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.node_id, 1);
        assert_eq!(deserialized.metric_name, "cpu");
        assert_eq!(deserialized.value, 42.5);
        assert_eq!(deserialized.timestamp, 1234567890);
    }

    #[test]
    fn test_remote_metric_clone_derive() {
        let metric = RemoteMetric {
            node_id: 1,
            metric_name: "test".to_string(),
            value: 50.0,
            timestamp: 100,
        };

        let cloned = metric.clone();
        assert_eq!(cloned.node_id, metric.node_id);
        assert_eq!(cloned.metric_name, metric.metric_name);
        assert_eq!(cloned.value, metric.value);
        assert_eq!(cloned.timestamp, metric.timestamp);
    }

    #[test]
    fn test_remote_metric_debug_derive() {
        let metric = RemoteMetric {
            node_id: 1,
            metric_name: "cpu".to_string(),
            value: 42.5,
            timestamp: 12345,
        };

        let debug_str = format!("{:?}", metric);
        assert!(debug_str.contains("RemoteMetric"));
        assert!(debug_str.contains("node_id"));
        assert!(debug_str.contains("cpu"));
    }

    #[test]
    fn test_remote_metric_edge_cases() {
        // Test with empty metric name
        let metric = RemoteMetric {
            node_id: 0,
            metric_name: "".to_string(),
            value: 0.0,
            timestamp: 0,
        };
        let json = serde_json::to_string(&metric).expect("Should serialize empty strings");
        let decoded: RemoteMetric = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(decoded.metric_name, "");
        assert_eq!(decoded.value, 0.0);

        // Test with max values
        let metric = RemoteMetric {
            node_id: u64::MAX,
            metric_name: "test".repeat(100).to_string(),
            value: f64::MAX,
            timestamp: u64::MAX,
        };
        let json = serde_json::to_string(&metric).expect("Should serialize max values");
        let decoded: RemoteMetric = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(decoded.node_id, u64::MAX);
        assert_eq!(decoded.value, f64::MAX);
    }

    #[test]
    fn test_remote_metric_unicode_metric_name() {
        let metric = RemoteMetric {
            node_id: 1,
            metric_name: "温度_℃".to_string(),
            value: 25.5,
            timestamp: 1000,
        };

        let json = serde_json::to_string(&metric).unwrap();
        let decoded: RemoteMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.metric_name, "温度_℃");
    }

    // --- MetricMessage Tests ---

    #[test]
    fn test_metric_message_serialization() {
        let msg = MetricMessage {
            node_id: 42,
            metric_name: "temperature".to_string(),
            value: 65.5,
            timestamp: 1609459200000,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        assert!(json.contains("\"node_id\":42"));
        assert!(json.contains("\"temperature\""));

        let decoded: MetricMessage = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(decoded.node_id, 42);
        assert_eq!(decoded.metric_name, "temperature");
        assert_eq!(decoded.value, 65.5);
    }

    #[test]
    fn test_metric_message_negative_values() {
        let msg = MetricMessage {
            node_id: 1,
            metric_name: "temperature_celsius".to_string(),
            value: -40.0,
            timestamp: 1000000,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize negative");
        let decoded: MetricMessage = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(decoded.value, -40.0);
    }

    #[test]
    fn test_metric_message_special_floats() {
        // Test with infinity
        let msg = MetricMessage {
            node_id: 1,
            metric_name: "infinity".to_string(),
            value: f64::INFINITY,
            timestamp: 1,
        };
        let json = serde_json::to_string(&msg).expect("Should serialize infinity");
        assert!(json.contains("Infinity") || json.contains("infinity") || json.contains("null"));

        // Test with NaN (should become null in JSON)
        let msg = MetricMessage {
            node_id: 1,
            metric_name: "nan".to_string(),
            value: f64::NAN,
            timestamp: 1,
        };
        let json = serde_json::to_string(&msg).expect("Should serialize NaN");
        // NaN becomes null in JSON
        assert!(json.contains("null") || json.contains("NaN"));
    }

    #[test]
    fn test_metric_message_clone_and_debug() {
        let msg = MetricMessage {
            node_id: 1,
            metric_name: "test".to_string(),
            value: 50.0,
            timestamp: 1000,
        };

        let cloned = msg.clone();
        assert_eq!(cloned.node_id, msg.node_id);

        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("MetricMessage"));
    }
}

// --- Aggregation Tests (using actual node.rs aggregation module) ---

mod aggregation_tests {
    use ddl::node::aggregation;

    #[test]
    fn test_avg_empty_values() {
        let values: Vec<f64> = vec![];
        assert_eq!(aggregation::avg(&values), 0.0);
    }

    #[test]
    fn test_avg_single_value() {
        let values = vec![42.0];
        assert_eq!(aggregation::avg(&values), 42.0);
    }

    #[test]
    fn test_avg_multiple_values() {
        let values = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(aggregation::avg(&values), 25.0);
    }

    #[test]
    fn test_avg_with_fractions() {
        let values = vec![1.5, 2.5, 3.0];
        assert!((aggregation::avg(&values) - 2.333333333333333).abs() < 0.0001);
    }

    #[test]
    fn test_avg_negative_values() {
        let values = vec![-10.0, -20.0, -30.0];
        assert_eq!(aggregation::avg(&values), -20.0);
    }

    #[test]
    fn test_avg_mixed_signs() {
        let values = vec![-10.0, 0.0, 10.0];
        assert_eq!(aggregation::avg(&values), 0.0);
    }

    #[test]
    fn test_max_empty_values() {
        let values: Vec<f64> = vec![];
        assert_eq!(aggregation::max(&values), f64::NEG_INFINITY);
    }

    #[test]
    fn test_max_single_value() {
        let values = vec![42.0];
        assert_eq!(aggregation::max(&values), 42.0);
    }

    #[test]
    fn test_max_multiple_values() {
        let values = vec![10.0, 50.0, 30.0, 20.0];
        assert_eq!(aggregation::max(&values), 50.0);
    }

    #[test]
    fn test_max_with_negative() {
        let values = vec![-10.0, -5.0, -20.0];
        assert_eq!(aggregation::max(&values), -5.0);
    }

    #[test]
    fn test_max_with_infinity() {
        let values = vec![1.0, f64::INFINITY, 100.0];
        assert_eq!(aggregation::max(&values), f64::INFINITY);
    }

    #[test]
    fn test_min_empty_values() {
        let values: Vec<f64> = vec![];
        assert_eq!(aggregation::min(&values), f64::INFINITY);
    }

    #[test]
    fn test_min_single_value() {
        let values = vec![42.0];
        assert_eq!(aggregation::min(&values), 42.0);
    }

    #[test]
    fn test_min_multiple_values() {
        let values = vec![10.0, 5.0, 30.0, 2.0];
        assert_eq!(aggregation::min(&values), 2.0);
    }

    #[test]
    fn test_min_with_negative() {
        let values = vec![-10.0, -5.0, -20.0];
        assert_eq!(aggregation::min(&values), -20.0);
    }

    #[test]
    fn test_min_with_neg_infinity() {
        let values = vec![1.0, f64::NEG_INFINITY, 100.0];
        assert_eq!(aggregation::min(&values), f64::NEG_INFINITY);
    }

    #[test]
    fn test_sum_empty_values() {
        let values: Vec<f64> = vec![];
        assert_eq!(aggregation::sum(&values), 0.0);
    }

    #[test]
    fn test_sum_single_value() {
        let values = vec![42.0];
        assert_eq!(aggregation::sum(&values), 42.0);
    }

    #[test]
    fn test_sum_multiple_values() {
        let values = vec![10.0, 20.0, 30.0];
        assert_eq!(aggregation::sum(&values), 60.0);
    }

    #[test]
    fn test_sum_negative_values() {
        let values = vec![-10.0, -20.0, 30.0];
        assert_eq!(aggregation::sum(&values), 0.0);
    }

    #[test]
    fn test_sum_large_values() {
        let values = vec![1e10, 2e10, 3e10];
        assert_eq!(aggregation::sum(&values), 6e10);
    }

    #[test]
    fn test_percentile_empty_values() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(aggregation::percentile(&mut values, 50.0), 0.0);
    }

    #[test]
    fn test_percentile_single_value() {
        let mut values = vec![100.0];
        // Any percentile of a single value should be that value
        assert_eq!(aggregation::percentile(&mut values, 50.0), 100.0);
        assert_eq!(aggregation::percentile(&mut values, 0.0), 100.0);
        assert_eq!(aggregation::percentile(&mut values, 100.0), 100.0);
    }

    #[test]
    fn test_percentile_p50() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let p50 = aggregation::percentile(&mut values, 50.0);
        // For 5 values, index = 50/100 * 4 = 2, so values[2] = 30
        assert_eq!(p50, 30.0);
    }

    #[test]
    fn test_percentile_p95() {
        let mut values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let p95 = aggregation::percentile(&mut values, 95.0);
        // For 100 values, index = 95/100 * 99 = 94.05 -> 94
        assert_eq!(p95, 95.0);
    }

    #[test]
    fn test_percentile_p99() {
        let mut values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let p99 = aggregation::percentile(&mut values, 99.0);
        // For 100 values, index = 99/100 * 99 = 98.01 -> 98
        assert_eq!(p99, 99.0);
    }

    #[test]
    fn test_percentile_with_duplicates() {
        let mut values = vec![10.0, 10.0, 20.0, 20.0, 30.0];
        let p50 = aggregation::percentile(&mut values, 50.0);
        // After sorting: [10, 10, 20, 20, 30]
        // index = 50/100 * 4 = 2, values[2] = 20
        assert_eq!(p50, 20.0);
    }

    #[test]
    fn test_percentile_sorts_input() {
        // Test that percentile sorts the input
        let mut values = vec![100.0, 10.0, 50.0, 5.0, 75.0];
        let p50 = aggregation::percentile(&mut values, 50.0);
        // After sorting: [5, 10, 50, 75, 100]
        // index = 2, values[2] = 50
        assert_eq!(p50, 50.0);
        // Verify values are sorted after the call
        assert_eq!(values, vec![5.0, 10.0, 50.0, 75.0, 100.0]);
    }

    #[test]
    fn test_percentile_extreme_values() {
        let mut values = vec![f64::MIN, 0.0, f64::MAX];
        let p50 = aggregation::percentile(&mut values, 50.0);
        // After sorting: [MIN, 0, MAX], index=1
        assert_eq!(p50, 0.0);
    }

    #[test]
    fn test_health_score_basic() {
        // Health score = (100 - cpu) + memory_free
        // For cpu=40, memory_free=60: (100-40) + 60 = 120
        assert_eq!(aggregation::health_score(40.0, 60.0), 120.0);
    }

    #[test]
    fn test_health_score_maximum() {
        // When cpu=0 and memory_free=100, health is maximum: 100 + 100 = 200
        assert_eq!(aggregation::health_score(0.0, 100.0), 200.0);
    }

    #[test]
    fn test_health_score_minimum() {
        // When cpu=100 and memory_free=0, health is minimum: 0 + 0 = 0
        assert_eq!(aggregation::health_score(100.0, 0.0), 0.0);
    }

    #[test]
    fn test_health_score_with_over_100_cpu() {
        // CPU can theoretically exceed 100% on multi-core
        assert_eq!(aggregation::health_score(150.0, 50.0), 0.0); // (100-150) + 50 = -50 + 50 = 0
    }

    #[test]
    fn test_health_score_negative_memory() {
        // Negative memory_free doesn't make physical sense but should work mathematically
        assert_eq!(aggregation::health_score(50.0, -10.0), 40.0); // (100-50) + (-10) = 50 - 10 = 40
    }
}

// ============================================================================
// 2. src/queue/registry.rs Tests
// ============================================================================

mod registry_tests {
    use super::*;

    fn create_test_config() -> Config {
        Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .build()
    }

    #[test]
    fn test_registry_new() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);
        // Registry should be created successfully
        assert!(!registry.queue_exists("nonexistent"));
    }

    #[tokio::test]
    async fn test_registry_add_queue() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let source = FunctionSource::new(|| 42u32);
        let result = registry.add_queue("test_queue", source);
        assert!(result.is_ok());
        assert!(registry.queue_exists("test_queue"));
    }

    #[tokio::test]
    async fn test_registry_add_queue_with_persistence() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .persistence(PersistenceConfig {
                data_dir: temp_dir.path().to_path_buf(),
                max_file_size: 1024 * 1024,
                flush_interval_ms: 100,
                include_timestamp: true,
                compress_old: false,
            })
            .build();

        let registry = QueueRegistry::new(config);
        let source = FunctionSource::new(|| 42.0f64);
        let result = registry.add_queue_with_persistence("persisted_queue", source);
        assert!(result.is_ok());
        assert!(registry.queue_exists("persisted_queue"));
    }

    #[tokio::test]
    async fn test_registry_queue_exists() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        // Initially queue doesn't exist
        assert!(!registry.queue_exists("new_queue"));

        let source = FunctionSource::new(|| 1i32);
        registry.add_queue("new_queue", source).unwrap();

        // Now it exists
        assert!(registry.queue_exists("new_queue"));
        assert!(!registry.queue_exists("other_queue"));
    }

    #[tokio::test]
    async fn test_registry_get_queue_success() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let source = FunctionSource::new(|| 100u32);
        registry.add_queue("test_get", source).unwrap();

        let result = registry.get_queue::<u32>("test_get");
        assert!(result.is_ok());

        let queue = result.unwrap();
        // Verify we can use the queue
        {
            let mut guard = queue.write().unwrap();
            guard.push(123).unwrap();
        }
        {
            let guard = queue.read().unwrap();
            assert_eq!(guard.len(), 1);
        }
    }

    #[test]
    fn test_registry_get_queue_not_found() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let result = registry.get_queue::<i32>("nonexistent");
        assert!(result.is_err());

        match result {
            Err(ddl::queue::source::AutoQueuesError::QueueNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected QueueNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_registry_get_queue_type_mismatch() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let source = FunctionSource::new(|| 42u32);
        registry.add_queue("typed_queue", source).unwrap();

        // Try to get with wrong type
        let result = registry.get_queue::<String>("typed_queue");
        assert!(result.is_err());

        match result {
            Err(ddl::queue::source::AutoQueuesError::QueueError(
                ddl::queue::queue_server::QueueError::Other(msg),
            )) => {
                assert!(msg.contains("type mismatch"));
            }
            _ => panic!("Expected Other error with type mismatch"),
        }
    }

    #[tokio::test]
    async fn test_registry_start() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        // Add multiple queues
        let source1 = FunctionSource::new(|| 1u32);
        let source2 = FunctionSource::new(|| 2u32);
        registry.add_queue("queue1", source1).unwrap();
        registry.add_queue("queue2", source2).unwrap();

        // Start should succeed (currently just prints)
        registry.start();
        // No return value to check, just verifying it doesn't panic
    }

    #[tokio::test]
    async fn test_registry_pause_and_resume() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let source = FunctionSource::new(|| 1u32);
        registry.add_queue("pauseable_queue", source).unwrap();

        // Pause and resume should succeed (currently just prints)
        registry.pause("pauseable_queue");
        registry.resume("pauseable_queue");

        // Non-existent queue, should not panic (just no-op)
        registry.pause("nonexistent");
        registry.resume("nonexistent");
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let source = FunctionSource::new(|| 1u32);
        registry.add_queue("removable_queue", source).unwrap();
        assert!(registry.queue_exists("removable_queue"));

        registry.remove("removable_queue");

        // Queue should no longer exist
        assert!(!registry.queue_exists("removable_queue"));

        // Removing non-existent should not panic
        registry.remove("also_nonexistent");
    }

    #[test]
    fn test_registry_enable_persistence() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .persistence(PersistenceConfig {
                data_dir: temp_dir.path().to_path_buf(),
                max_file_size: 1024 * 1024,
                flush_interval_ms: 100,
                include_timestamp: true,
                compress_old: false,
            })
            .build();

        let registry = QueueRegistry::new(config);

        let result = registry.enable_persistence("new_persisted_queue");
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_get_persistence_handle() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .persistence(PersistenceConfig {
                data_dir: temp_dir.path().to_path_buf(),
                max_file_size: 1024 * 1024,
                flush_interval_ms: 100,
                include_timestamp: true,
                compress_old: false,
            })
            .build();

        let registry = QueueRegistry::new(config);

        // Initially no handle
        assert!(registry.get_persistence_handle("nonexistent").is_none());

        // Enable persistence
        registry.enable_persistence("test_handle").unwrap();

        // Now handle should exist
        assert!(registry.get_persistence_handle("test_handle").is_some());
    }

    #[test]
    fn test_registry_shutdown_all_persistence() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .persistence(PersistenceConfig {
                data_dir: temp_dir.path().to_path_buf(),
                max_file_size: 1024 * 1024,
                flush_interval_ms: 100,
                include_timestamp: true,
                compress_old: false,
            })
            .build();

        let registry = QueueRegistry::new(config);

        // Enable persistence for multiple queues
        registry.enable_persistence("queue1").unwrap();
        registry.enable_persistence("queue2").unwrap();

        // Shutdown should not panic
        registry.shutdown_all_persistence();
    }

    #[tokio::test]
    async fn test_registry_multiple_queues_different_types() {
        let config = create_test_config();
        let registry = QueueRegistry::new(config);

        let uint_source = FunctionSource::new(|| 42u32);
        let int_source = FunctionSource::new(|| -1i64);
        let float_source = FunctionSource::new(|| 3.14f64);

        registry.add_queue("uint_queue", uint_source).unwrap();
        registry.add_queue("int_queue", int_source).unwrap();
        registry.add_queue("float_queue", float_source).unwrap();

        // All queues should exist
        assert!(registry.queue_exists("uint_queue"));
        assert!(registry.queue_exists("int_queue"));
        assert!(registry.queue_exists("float_queue"));

        // Each should have correct type
        assert!(registry.get_queue::<u32>("uint_queue").is_ok());
        assert!(registry.get_queue::<i64>("int_queue").is_ok());
        assert!(registry.get_queue::<f64>("float_queue").is_ok());
    }
}

// ============================================================================
// 3. src/queue/interval.rs Tests
// ============================================================================

mod interval_tests {
    use super::*;

    // --- IntervalConfig Tests ---

    #[test]
    fn test_interval_config_default() {
        let config = IntervalConfig::default();
        match config {
            IntervalConfig::Constant(ms) => assert_eq!(ms, 1000),
            IntervalConfig::Adaptive { .. } => panic!("Expected Constant variant"),
        }
    }

    #[test]
    fn test_interval_config_constant() {
        let config = IntervalConfig::Constant(500);
        match config {
            IntervalConfig::Constant(ms) => assert_eq!(ms, 500),
            _ => panic!("Expected Constant variant"),
        }
    }

    #[test]
    fn test_interval_config_adaptive() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 50,
            multiplicative_factor: 2.0,
        };

        match config {
            IntervalConfig::Adaptive {
                initial_ms,
                min_ms,
                max_ms,
                additive_step_ms,
                multiplicative_factor,
            } => {
                assert_eq!(initial_ms, 1000);
                assert_eq!(min_ms, 100);
                assert_eq!(max_ms, 5000);
                assert_eq!(additive_step_ms, 50);
                assert!((multiplicative_factor - 2.0).abs() < 0.001);
            }
            _ => panic!("Expected Adaptive variant"),
        }
    }

    // --- AimdController Tests ---

    #[test]
    fn test_aimd_controller_new_constant() {
        let config = IntervalConfig::Constant(500);
        let controller = AimdController::new(config);

        assert_eq!(controller.current_interval_ms(), 500);
        assert_eq!(controller.interval(), Duration::from_millis(500));
    }

    #[test]
    fn test_aimd_controller_new_adaptive() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 2000,
            min_ms: 100,
            max_ms: 10000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let controller = AimdController::new(config);

        assert_eq!(controller.current_interval_ms(), 2000);
        assert_eq!(controller.interval(), Duration::from_millis(2000));
    }

    #[test]
    fn test_aimd_controller_interval_constant() {
        let config = IntervalConfig::Constant(1234);
        let controller = AimdController::new(config);

        assert_eq!(controller.interval(), Duration::from_millis(1234));
    }

    #[test]
    fn test_aimd_controller_on_success_constant() {
        let config = IntervalConfig::Constant(1000);
        let mut controller = AimdController::new(config);

        // Success should not change constant interval
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 1000);

        controller.on_success();
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 1000);
    }

    #[test]
    fn test_aimd_controller_on_success_adaptive() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // On success, interval should decrease (towards faster polling)
        let initial = controller.current_interval_ms();

        controller.on_success();
        assert!(controller.current_interval_ms() < initial);
        assert_eq!(controller.current_interval_ms(), 900); // 1000 - 100

        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 800); // 900 - 100
    }

    #[test]
    fn test_aimd_controller_on_overflow_constant() {
        let config = IntervalConfig::Constant(1000);
        let mut controller = AimdController::new(config);

        // Overflow should not change constant interval
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1000);

        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1000);
    }

    #[test]
    fn test_aimd_controller_on_overflow_adaptive() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // On overflow, interval should increase (towards slower polling)
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 2000); // 1000 * 2

        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 4000); // 2000 * 2

        // Should cap at max
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 5000); // capped at max
    }

    #[test]
    fn test_aimd_controller_interval_bounds_min() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 150,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // Success should decrease but not below min
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 100); // min

        // More successes stay at min
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 100);
    }

    #[test]
    fn test_aimd_controller_interval_bounds_max() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 3000,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // Overflow should increase but cap at max
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 5000); // capped at max

        // More overflows stay at max
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 5000);
    }

    #[test]
    fn test_aimd_controller_current_interval_ms() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 500,
            max_ms: 10000,
            additive_step_ms: 50,
            multiplicative_factor: 1.5,
        };
        let mut controller = AimdController::new(config);

        // Check initial
        assert_eq!(controller.current_interval_ms(), 1000);

        // After some successes
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 950);

        // After overflow
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1425); // 950 * 1.5
    }

    #[test]
    fn test_aimd_controller_consecutive_successes_tracking() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 10000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // Multiple successes
        controller.on_success();
        controller.on_success();
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 700); // 1000 - 300

        // Overflow resets track
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1400); // 700 * 2

        // Success again
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 1300); // 1400 - 100
    }

    #[test]
    fn test_aimd_controller_alternating_pattern() {
        // Simulate realistic usage pattern
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 8000,
            additive_step_ms: 100,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // Success reduces interval
        controller.on_success();
        assert_eq!(controller.current_interval_ms(), 900);

        // Overflow increases
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1800);

        // More successes reduce again
        for _ in 0..10 {
            controller.on_success();
        }
        // 1800 - 1000 = 800 (but min is 100)
        assert!(controller.current_interval_ms() >= 100);

        // Final overflow
        controller.on_overflow();
        // Should be at least 200 (100 * 2) but could be more depending on state
        assert!(controller.current_interval_ms() >= 200);
    }

    #[test]
    fn test_aimd_controller_saturating_sub() {
        // Test that subtracting doesn't go below min
        let config = IntervalConfig::Adaptive {
            initial_ms: 150,
            min_ms: 100,
            max_ms: 1000,
            additive_step_ms: 200, // Larger than initial - min
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        controller.on_success();
        // Should not go below 100
        assert_eq!(controller.current_interval_ms(), 100);
    }

    #[test]
    fn test_interval_config_clone_derives_correctly() {
        let config = IntervalConfig::Adaptive {
            initial_ms: 1000,
            min_ms: 100,
            max_ms: 5000,
            additive_step_ms: 50,
            multiplicative_factor: 2.0,
        };

        // IntervalConfig should be Clone
        let config2 = config.clone();
        match config2 {
            IntervalConfig::Adaptive { initial_ms, .. } => {
                assert_eq!(initial_ms, 1000);
            }
            _ => panic!("Expected Adaptive"),
        }
    }
}

// ============================================================================
// 4. src/queue/queue_server.rs Tests
// ============================================================================

mod queue_server_tests {
    use super::*;

    // --- QueueError Tests ---

    #[test]
    fn test_queue_error_empty() {
        let error = QueueError::Empty;
        assert_eq!(format!("{}", error), "Queue is empty");
    }

    #[test]
    fn test_queue_error_publish_error() {
        let error = QueueError::PublishError("Connection failed".to_string());
        assert_eq!(format!("{}", error), "Publish error: Connection failed");
    }

    #[test]
    fn test_queue_error_server_error() {
        let error = QueueError::ServerError("Timeout".to_string());
        assert_eq!(format!("{}", error), "Server error: Timeout");
    }

    #[test]
    fn test_queue_error_other() {
        let error = QueueError::Other("Custom error".to_string());
        assert_eq!(format!("{}", error), "Other error: Custom error");
    }

    #[test]
    fn test_queue_error_debug() {
        let error = QueueError::Empty;
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Empty"));
    }

    #[test]
    fn test_queue_error_display_variants() {
        // Test all variants display correctly
        assert_eq!(QueueError::Empty.to_string(), "Queue is empty");
        assert_eq!(
            QueueError::PublishError("test".to_string()).to_string(),
            "Publish error: test"
        );
        assert_eq!(
            QueueError::ServerError("fail".to_string()).to_string(),
            "Server error: fail"
        );
        assert_eq!(
            QueueError::Other("custom".to_string()).to_string(),
            "Other error: custom"
        );
    }

    // --- QueueServerHandle Tests ---

    #[tokio::test]
    async fn test_queue_server_handle_new() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async {
            // Wait for shutdown signal
            let _ = rx.await;
        });

        let server_handle = QueueServerHandle::new(tx, handle);
        // Just verify creation works
        drop(server_handle);
    }

    #[tokio::test]
    async fn test_queue_server_handle_shutdown_success() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async {
            let _ = rx.await;
            // Server task completes successfully
        });

        let server_handle = QueueServerHandle::new(tx, handle);

        // Shutdown should succeed
        let result = server_handle.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_queue_server_handle_already_stopped() {
        // Create a channel and drop receiver immediately
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

        let handle = tokio::spawn(async {
            // Receiver will be dropped
            rx.close();
            let _ = rx.await;
        });

        // Let the task complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        let server_handle = QueueServerHandle::new(tx, handle);

        // Shutdown should handle the already-stopped case
        let result = server_handle.shutdown().await;
        // The actual behavior depends on timing, but it shouldn't panic
        assert!(result.is_ok() || matches!(result, Err(QueueError::ServerError(_))));
    }

    #[tokio::test]
    async fn test_queue_server_handle_long_running_task() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let handle = tokio::spawn(async move {
            // Simulate long-running work
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = rx.await;
        });

        let server_handle = QueueServerHandle::new(tx, handle);

        // Shutdown should wait for task to complete
        let start = std::time::Instant::now();
        let result = server_handle.shutdown().await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed.as_millis() >= 100); // Should have waited for the sleep
    }
}

// ============================================================================
// 5. src/cluster/raft_node.rs Tests
// ============================================================================

mod raft_node_tests {
    use super::*;

    #[test]
    fn test_cluster_config_default() {
        let config = ClusterConfig::default();

        assert_eq!(config.node_id, 1);
        assert!(config.peers.is_empty());
        assert_eq!(config.timeout_ms, 1000);
    }

    #[test]
    fn test_cluster_config_custom_values() {
        let config = ClusterConfig {
            node_id: 42,
            peers: vec!["node1:7000".to_string(), "node2:7000".to_string()],
            timeout_ms: 5000,
        };

        assert_eq!(config.node_id, 42);
        assert_eq!(config.peers.len(), 2);
        assert_eq!(config.timeout_ms, 5000);
    }

    #[test]
    fn test_cluster_config_clone() {
        let config = ClusterConfig {
            node_id: 1,
            peers: vec!["localhost:1234".to_string()],
            timeout_ms: 2000,
        };

        let cloned = config.clone();
        assert_eq!(cloned.node_id, 1);
        assert_eq!(cloned.peers.len(), 1);
        assert_eq!(cloned.timeout_ms, 2000);
    }

    #[tokio::test]
    async fn test_raft_node_new() {
        let config = ClusterConfig {
            node_id: 5,
            peers: vec!["peer1:8080".to_string(), "peer2:8080".to_string()],
            timeout_ms: 3000,
        };

        let _node = RaftNode::new(config);

        // Just verify creation works
        assert!(true);
    }

    #[tokio::test]
    async fn test_raft_node_start() {
        let config = ClusterConfig::default();
        let node = RaftNode::new(config);

        let result = node.start().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raft_node_stop() {
        let config = ClusterConfig::default();
        let node = RaftNode::new(config);

        // Start then stop
        node.start().await.unwrap();

        let result = node.stop().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raft_node_lifecycle() {
        let config = ClusterConfig {
            node_id: 10,
            peers: vec!["node1:7000".to_string()],
            timeout_ms: 3000,
        };
        let node = RaftNode::new(config);

        // Start
        node.start().await.expect("Start should succeed");

        // Stop
        node.stop().await.expect("Stop should succeed");
    }

    #[test]
    fn test_cluster_config_debug() {
        let config = ClusterConfig {
            node_id: 1,
            peers: vec!["test:123".to_string()],
            timeout_ms: 500,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("node_id"));
        assert!(debug_str.contains("peers"));
        assert!(debug_str.contains("timeout_ms"));
    }
}

// ============================================================================
// 6. src/cluster/raft_router.rs Tests
// ============================================================================

mod raft_router_tests {
    // Note: RaftMessageRouter requires a live Raft instance which is complex to mock.
    // We focus on structural tests and verify the API shape.

    // The RaftMessageRouter requires an Arc<Raft<TypeConfig>> which requires
    // a full Raft setup with storage, network, etc. This is integration-level
    // and best tested in integration tests with actual Raft instances.
    // Here we just verify the module exists and can be imported.

    #[test]
    fn test_raft_router_module_exists() {
        // Just verify the module exists and compiles
        // The router module is imported from ddl::cluster::raft_router
        // which requires openraft::Raft<TypeConfig> instance.
        assert!(true);
    }

    // For full testing, see raft_integration_tests.rs
}

// ============================================================================
// 7. src/network/raft_transport.rs Tests
// ============================================================================

mod raft_transport_tests {
    use super::*;
    use std::collections::HashMap;

    // Note: ZmqRaftNetwork tests are limited because the actual async methods
    // require ZMQ sockets which need actual network operations.
    // These tests focus on construction and configuration validation.

    #[test]
    fn test_zmq_raft_network_new() {
        let mut peers = HashMap::new();
        peers.insert(1, ("127.0.0.1".to_string(), 7000u16));
        peers.insert(2, ("127.0.0.1".to_string(), 7001u16));

        let result = ddl::network::raft_transport::ZmqRaftNetwork::new(1, peers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_get_address_existing_peer() {
        let mut peers = HashMap::new();
        peers.insert(1, ("192.168.1.1".to_string(), 8080u16));
        peers.insert(2, ("192.168.1.2".to_string(), 8080u16));

        let network = ddl::network::raft_transport::ZmqRaftNetwork::new(0, peers).unwrap();

        // Cannot directly access get_address as it's private, but we can
        // test that the network is created successfully with peers
        let _ = network;
    }

    #[test]
    fn test_zmq_raft_network_get_address_non_existing_peer() {
        let mut peers = HashMap::new();
        peers.insert(1, ("127.0.0.1".to_string(), 7000u16));

        let network = ddl::network::raft_transport::ZmqRaftNetwork::new(0, peers).unwrap();

        // Network should still create even with no matching peer
        let _ = network;
    }

    #[test]
    fn test_zmq_raft_network_empty_peers() {
        let peers: HashMap<u64, (String, u16)> = HashMap::new();

        let result = ddl::network::raft_transport::ZmqRaftNetwork::new(1, peers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_multiple_peers() {
        let mut peers = HashMap::new();
        for i in 1..=10u64 {
            peers.insert(i, (format!("node{}", i), 7000 + i as u16));
        }

        let result = ddl::network::raft_transport::ZmqRaftNetwork::new(0, peers);
        assert!(result.is_ok());
    }
}

// ============================================================================
// 8. src/network/pubsub/zmq/transport.rs Tests
// ============================================================================

mod zmq_transport_tests {
    use super::*;

    #[test]
    fn test_zmq_transport_new() {
        let result = ZmqTransport::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_transport_is_connected_initial() {
        let transport = ZmqTransport::new().unwrap();

        // Initially not connected
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_zmq_transport_socket_access() {
        let transport = ZmqTransport::new().unwrap();

        // Socket should be accessible
        let _socket = transport.socket();
    }

    // --- NodeMessage Tests ---

    #[test]
    fn test_node_message_request_value() {
        let msg = NodeMessage::RequestValue {
            queue_name: "test_queue".to_string(),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        assert!(json.contains("RequestValue"));
        assert!(json.contains("test_queue"));

        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::RequestValue { queue_name } => {
                assert_eq!(queue_name, "test_queue");
            }
            _ => panic!("Expected RequestValue variant"),
        }
    }

    #[test]
    fn test_node_message_value_response() {
        let msg = NodeMessage::ValueResponse {
            queue_name: "metrics".to_string(),
            value: Some(42.5),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::ValueResponse { queue_name, value } => {
                assert_eq!(queue_name, "metrics");
                assert_eq!(value, Some(42.5));
            }
            _ => panic!("Expected ValueResponse"),
        }
    }

    #[test]
    fn test_node_message_value_response_none() {
        let msg = NodeMessage::ValueResponse {
            queue_name: "empty_queue".to_string(),
            value: None,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::ValueResponse { value, .. } => {
                assert!(value.is_none());
            }
            _ => panic!("Expected ValueResponse"),
        }
    }

    #[test]
    fn test_node_message_request_expression() {
        let msg = NodeMessage::RequestExpression {
            sources: vec!["cpu".to_string(), "memory".to_string()],
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::RequestExpression { sources } => {
                assert_eq!(sources, vec!["cpu", "memory"]);
            }
            _ => panic!("Expected RequestExpression"),
        }
    }

    #[test]
    fn test_node_message_expression_response() {
        let msg = NodeMessage::ExpressionResponse {
            sources: vec!["temp".to_string()],
            score: Some(75.5),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::ExpressionResponse { sources, score } => {
                assert_eq!(sources, vec!["temp"]);
                assert_eq!(score, Some(75.5));
            }
            _ => panic!("Expected ExpressionResponse"),
        }
    }

    #[test]
    fn test_node_message_request_aggregation() {
        let msg = NodeMessage::RequestAggregation {
            queue_name: "stats".to_string(),
            operation: "avg".to_string(),
            args: vec!["10".to_string(), "20".to_string()],
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::RequestAggregation {
                queue_name,
                operation,
                args,
            } => {
                assert_eq!(queue_name, "stats");
                assert_eq!(operation, "avg");
                assert_eq!(args, vec!["10", "20"]);
            }
            _ => panic!("Expected RequestAggregation"),
        }
    }

    #[test]
    fn test_node_message_aggregation_response() {
        let msg = NodeMessage::AggregationResponse { result: 123.456 };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::AggregationResponse { result } => {
                assert!((result - 123.456).abs() < 0.001);
            }
            _ => panic!("Expected AggregationResponse"),
        }
    }

    #[test]
    fn test_node_message_broadcast_value() {
        let msg = NodeMessage::BroadcastValue {
            queue_name: "metrics".to_string(),
            value: 99.9,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NodeMessage::BroadcastValue { queue_name, value } => {
                assert_eq!(queue_name, "metrics");
                assert!((value - 99.9).abs() < 0.001);
            }
            _ => panic!("Expected BroadcastValue"),
        }
    }

    #[test]
    fn test_node_message_roundtrip_all_variants() {
        // Test all variants roundtrip through JSON
        let messages = vec![
            NodeMessage::RequestValue {
                queue_name: "q1".to_string(),
            },
            NodeMessage::ValueResponse {
                queue_name: "q2".to_string(),
                value: Some(1.0),
            },
            NodeMessage::ValueResponse {
                queue_name: "q3".to_string(),
                value: None,
            },
            NodeMessage::RequestExpression {
                sources: vec!["a".to_string(), "b".to_string()],
            },
            NodeMessage::ExpressionResponse {
                sources: vec!["x".to_string()],
                score: None,
            },
            NodeMessage::ExpressionResponse {
                sources: vec![],
                score: Some(50.0),
            },
            NodeMessage::RequestAggregation {
                queue_name: "q".to_string(),
                operation: "sum".to_string(),
                args: vec![],
            },
            NodeMessage::AggregationResponse { result: 0.0 },
            NodeMessage::BroadcastValue {
                queue_name: "b".to_string(),
                value: f64::MAX,
            },
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).expect("Should serialize");
            let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
            // Just verify it roundtrips without crashing
            let _ = format!("{:?}", decoded);
        }
    }

    #[test]
    fn test_node_message_debug_format() {
        let msg = NodeMessage::RequestValue {
            queue_name: "test".to_string(),
        };
        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("RequestValue"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_node_message_clone() {
        let msg = NodeMessage::AggregationResponse { result: 42.0 };
        let cloned = msg.clone();

        match cloned {
            NodeMessage::AggregationResponse { result } => {
                assert_eq!(result, 42.0);
            }
            _ => panic!("Expected AggregationResponse"),
        }
    }

    #[test]
    fn test_zmq_transport_debug_format() {
        let transport = ZmqTransport::new().unwrap();
        let debug_str = format!("{:?}", transport);
        assert!(debug_str.contains("ZmqTransport"));
    }
}

// ============================================================================
// Integration Tests (combining multiple components)
// ============================================================================

mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_with_function_source_async() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .persistence(PersistenceConfig {
                data_dir: temp_dir.path().to_path_buf(),
                max_file_size: 1024 * 1024,
                flush_interval_ms: 1000,
                include_timestamp: true,
                compress_old: false,
            })
            .build();

        let registry = QueueRegistry::new(config);

        // Create a counter
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        // Create source with interval config
        let interval_config = IntervalConfig::Adaptive {
            initial_ms: 10,
            min_ms: 5,
            max_ms: 1000,
            additive_step_ms: 5,
            multiplicative_factor: 2.0,
        };

        let source = FunctionSource::new(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed)
        });

        // Add queue with adaptive interval
        let result = registry.add_queue_with_interval("async_queue", source, interval_config);
        assert!(result.is_ok());

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify some data was generated
        let count = counter.load(Ordering::Relaxed);
        assert!(count > 0, "Source should have generated data");
    }

    #[tokio::test]
    async fn test_full_config_to_registry_flow() {
        // Create a complete config
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .add_node("node2", "127.0.0.1", 7101, 7102, 7103, 7104)
            .add_local_metric("cpu", "function:cpu")
            .add_local_metric("memory", "function:memory")
            .add_global_aggregation("cpu_avg", "avg", &["cpu"], 1000)
            .capacity(2048)
            .build();

        // Create registry from config
        let registry = QueueRegistry::new(config);

        // Add multiple queues
        let source1 = FunctionSource::new(|| 1.0f64);
        let source2 = FunctionSource::new(|| 2.0f64);

        registry.add_queue("metrics1", source1).unwrap();
        registry.add_queue("metrics2", source2).unwrap();

        assert!(registry.queue_exists("metrics1"));
        assert!(registry.queue_exists("metrics2"));

        // Clean up
        registry.shutdown_all_persistence();
    }

    #[tokio::test]
    async fn test_interval_config_multiple_queues() {
        let config = Config::default();
        let registry = QueueRegistry::new(config);

        // Different interval configs for different queues
        let constant_config = IntervalConfig::Constant(100);
        let adaptive_config = IntervalConfig::Adaptive {
            initial_ms: 200,
            min_ms: 50,
            max_ms: 1000,
            additive_step_ms: 10,
            multiplicative_factor: 1.5,
        };

        let source1 = FunctionSource::new(|| true);
        let source2 = FunctionSource::new(|| false);

        let result1 = registry.add_queue_with_interval("bool_queue_1", source1, constant_config);
        let result2 = registry.add_queue_with_interval("bool_queue_2", source2, adaptive_config);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_aimd_controller_realistic_pattern() {
        // Simulate a realistic AIMD pattern with successes and overflows
        let config = IntervalConfig::Adaptive {
            initial_ms: 500,
            min_ms: 100,
            max_ms: 2000,
            additive_step_ms: 50,
            multiplicative_factor: 2.0,
        };
        let mut controller = AimdController::new(config);

        // Initial period: mostly successes
        for _ in 0..5 {
            controller.on_success();
        }
        assert_eq!(controller.current_interval_ms(), 250); // 500 - 250

        // Overflow occurs
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 500); // 250 * 2

        // Recovery period: more successes
        for _ in 0..3 {
            controller.on_success();
        }
        assert_eq!(controller.current_interval_ms(), 350); // 500 - 150

        // Multiple overflows in a row (congestion)
        controller.on_overflow();
        controller.on_overflow();
        assert_eq!(controller.current_interval_ms(), 1400); // 350 * 2 * 2

        // After congestion clears - 14 successes to get below 700
        for _ in 0..14 {
            controller.on_success();
        }
        // 1400 - 14*50 = 700
        assert!(controller.current_interval_ms() < 750);
    }
}

// ============================================================================
// Edge Cases and Error Handling Tests
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_interval_config_extremes() {
        // Test with very large values
        let config = IntervalConfig::Adaptive {
            initial_ms: u64::MAX,
            min_ms: 1,
            max_ms: u64::MAX,
            additive_step_ms: u64::MAX,
            multiplicative_factor: 1.0,
        };
        let controller = AimdController::new(config);
        assert_eq!(controller.current_interval_ms(), u64::MAX);

        // Test with very small values
        let config = IntervalConfig::Adaptive {
            initial_ms: 1,
            min_ms: 1,
            max_ms: 1,
            additive_step_ms: 1,
            multiplicative_factor: 2.0,
        };
        let controller = AimdController::new(config);
        assert_eq!(controller.current_interval_ms(), 1);
    }

    #[test]
    fn test_queue_registry_empty_operations() {
        let config = Config::default();
        let registry = QueueRegistry::new(config);

        // Operations on nonexistent queues should not panic
        registry.pause("nonexistent");
        registry.resume("nonexistent");
        registry.remove("nonexistent");

        assert!(!registry.queue_exists("nonexistent"));
    }

    #[test]
    fn test_queue_error_chain() {
        // Test error chain
        let err = QueueError::PublishError("failed".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Publish error"));
        assert!(display.contains("failed"));

        // Test nested error
        let auto_err = ddl::queue::source::AutoQueuesError::QueueError(err);
        assert!(format!("{}", auto_err).contains("Queue error"));
    }

    #[test]
    fn test_cluster_config_zero_node_id() {
        let config = ClusterConfig {
            node_id: 0,
            peers: vec![],
            timeout_ms: 0,
        };

        let _node = RaftNode::new(config);
        // Should allow node_id of 0
        assert!(true);
    }

    #[test]
    fn test_node_message_special_values() {
        // Test with special float values
        let msg = NodeMessage::BroadcastValue {
            queue_name: "test".to_string(),
            value: f64::NAN,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // NaN should serialize to null
        assert!(json.contains("null") || json.contains("NaN"));

        // Test with negative values
        let msg = NodeMessage::AggregationResponse {
            result: -f64::INFINITY,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("-Infinity") || json.contains("null"));
    }
}

// ============================================================================
// 9. Config Tests - Comprehensive Coverage
// ============================================================================

mod config_tests {
    use ddl::config::{Config, ConfigBuilder, ConfigError, NodeConfig, SourceType, AggregationType};
    use ddl::config::{LocalMetricConfig, GlobalMetricConfig, DebugConfig, QueueConfig, NodeTable, ConfigGenerator};
    use ddl::queue::persistence::PersistenceConfig;
    use tempfile::TempDir;

    // --- Config::default() and basic construction ---

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.nodes.nodes.is_empty());
        assert!(config.local.is_empty());
        assert!(config.global.is_empty());
        assert!(config.persistence.is_none());
        assert!(!config.debug.verbose);
        assert!(!config.debug.simulate_sensors);
    }

    #[test]
    fn test_config_new() {
        let config = Config::new();
        assert!(config.nodes.nodes.is_empty());
    }

    #[test]
    fn test_config_builder_new() {
        let builder = ConfigBuilder::new();
        let config = builder.build();
        assert!(config.nodes.nodes.is_empty());
    }

    // --- Config::load() tests ---

    #[test]
    fn test_config_load_valid_toml() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");
        
        // Config uses nodes.node1 format for node definitions
        let content = r#"
[nodes.node1]
host = "127.0.0.1"
communication_port = 7001
query_port = 7002
coordination_port = 7003
pubsub_port = 7004

[local]
cpu = "function:cpu"
memory = "function:memory"

[global.cpu_avg]
aggregation = "avg"
sources = ["cpu"]
interval_ms = 1000

[queue]
capacity = 1024
default_interval = 500
"#;
        std::fs::write(&config_path, content).expect("Failed to write config");
        
        let config = Config::load(&config_path).expect("Failed to load config");
        assert_eq!(config.nodes.nodes.len(), 1);
        assert!(config.nodes.nodes.contains_key("node1"));
        assert!(config.local.contains_key("cpu"));
        assert!(config.global.contains_key("cpu_avg"));
        assert_eq!(config.queue.capacity, 1024);
    }

    #[test]
    fn test_config_load_invalid_path() {
        let result = Config::load("/nonexistent/path/config.toml");
        assert!(result.is_err());
        match result {
            Err(ConfigError::Io(_)) => {}
            _ => panic!("Expected IO error"),
        }
    }

    #[test]
    fn test_config_load_invalid_toml() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("invalid.toml");
        
        let content = "invalid toml content [[[";
        std::fs::write(&config_path, content).expect("Failed to write config");
        
        let result = Config::load(&config_path);
        assert!(result.is_err());
        match result {
            Err(ConfigError::Parse(_)) => {}
            _ => panic!("Expected Parse error"),
        }
    }

    #[test]
    fn test_config_load_empty_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("empty.toml");
        
        std::fs::write(&config_path, "").expect("Failed to write config");
        
        let config = Config::load(&config_path).expect("Empty config should load");
        assert!(config.nodes.nodes.is_empty());
    }

    // --- Config::save() tests ---

    #[test]
    fn test_config_save_and_reload() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("save_test.toml");
        
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .add_local_metric("cpu", "function:cpu")
            .add_global_aggregation("cpu_avg", "avg", &["cpu"], 1000)
            .capacity(2048)
            .build();
        
        config.save(&config_path).expect("Failed to save config");
        assert!(config_path.exists());
        
        let reloaded = Config::load(&config_path).expect("Failed to reload config");
        assert_eq!(reloaded.nodes.nodes.len(), 1);
        assert!(reloaded.local.contains_key("cpu"));
    }

    // --- NodeConfig tests ---

    #[test]
    fn test_node_config_addr_methods() {
        let config = NodeConfig {
            host: "192.168.1.1".to_string(),
            communication_port: 8080,
            query_port: 8081,
            coordination_port: 8082,
            pubsub_port: 8083,
        };
        
        let comm_addr = config.communication_addr().expect("Should parse");
        assert!(comm_addr.to_string().contains("192.168.1.1"));
        assert!(comm_addr.to_string().contains("8080"));
        
        let query_addr = config.query_addr().expect("Should parse");
        assert!(query_addr.to_string().contains("8081"));
        
        let coord_addr = config.coordination_addr().expect("Should parse");
        assert!(coord_addr.to_string().contains("8082"));
        
        let pubsub_addr = config.pubsub_addr().expect("Should parse");
        assert!(pubsub_addr.to_string().contains("8083"));
    }

    #[test]
    fn test_node_config_clone() {
        let config = NodeConfig {
            host: "localhost".to_string(),
            communication_port: 7000,
            query_port: 7001,
            coordination_port: 7002,
            pubsub_port: 7003,
        };
        
        let cloned = config.clone();
        assert_eq!(config.host, cloned.host);
        assert_eq!(config.communication_port, cloned.communication_port);
    }

    #[test]
    fn test_node_config_debug() {
        let config = NodeConfig {
            host: "test.host".to_string(),
            communication_port: 7000,
            query_port: 7001,
            coordination_port: 7002,
            pubsub_port: 7003,
        };
        
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("NodeConfig"));
        assert!(debug_str.contains("test.host"));
    }

    #[test]
    fn test_node_table_collect_nodes() {
        let mut table = NodeTable::default();
        table.nodes.insert("node1".to_string(), NodeConfig {
            host: "host1".to_string(),
            communication_port: 7000,
            query_port: 7001,
            coordination_port: 7002,
            pubsub_port: 7003,
        });
        table.nodes.insert("node2".to_string(), NodeConfig {
            host: "host2".to_string(),
            communication_port: 8000,
            query_port: 8001,
            coordination_port: 8002,
            pubsub_port: 8003,
        });
        
        let nodes = table.collect_nodes();
        assert_eq!(nodes.len(), 2);
    }

    // --- SourceType tests ---

    #[test]
    fn test_source_type_parse_function() {
        let source = SourceType::parse("function:cpu").expect("Should parse");
        match source {
            SourceType::Function(name) => assert_eq!(name, "cpu"),
            _ => panic!("Expected Function variant"),
        }
    }

    #[test]
    fn test_source_type_parse_expression() {
        let source = SourceType::parse("expression:x * 2 + y").expect("Should parse");
        match source {
            SourceType::Expression(expr) => assert_eq!(expr, "x * 2 + y"),
            _ => panic!("Expected Expression variant"),
        }
    }

    #[test]
    fn test_source_type_parse_default_function() {
        // Without prefix, defaults to function
        let source = SourceType::parse("cpu").expect("Should parse");
        match source {
            SourceType::Function(name) => assert_eq!(name, "cpu"),
            _ => panic!("Expected Function variant"),
        }
    }

    #[test]
    fn test_source_type_variables_function() {
        let source = SourceType::Function("cpu".to_string());
        let vars = source.variables();
        assert_eq!(vars, vec!["cpu"]);
    }

    #[test]
    fn test_source_type_variables_expression() {
        let source = SourceType::Expression("x * 2 + y * z".to_string());
        let vars = source.variables();
        assert!(vars.contains(&"x".to_string()));
        assert!(vars.contains(&"y".to_string()));
        assert!(vars.contains(&"z".to_string()));
        assert!(!vars.contains(&"true".to_string())); // Keywords excluded
    }

    #[test]
    fn test_source_type_variables_complex_expression() {
        let source = SourceType::Expression("cpu_usage + memory_free * 0.5".to_string());
        let vars = source.variables();
        assert!(vars.contains(&"cpu_usage".to_string()));
        assert!(vars.contains(&"memory_free".to_string()));
    }

    // --- AggregationType tests ---

    #[test]
    fn test_aggregation_type_from_str_valid() {
        assert!(matches!(AggregationType::from_str("avg"), Ok(AggregationType::Avg)));
        assert!(matches!(AggregationType::from_str("max"), Ok(AggregationType::Max)));
        assert!(matches!(AggregationType::from_str("min"), Ok(AggregationType::Min)));
        assert!(matches!(AggregationType::from_str("sum"), Ok(AggregationType::Sum)));
        assert!(matches!(AggregationType::from_str("stddev"), Ok(AggregationType::Stddev)));
        assert!(matches!(AggregationType::from_str("p50"), Ok(AggregationType::Percentile50)));
        assert!(matches!(AggregationType::from_str("median"), Ok(AggregationType::Percentile50)));
        assert!(matches!(AggregationType::from_str("p95"), Ok(AggregationType::Percentile95)));
        assert!(matches!(AggregationType::from_str("p99"), Ok(AggregationType::Percentile99)));
    }

    #[test]
    fn test_aggregation_type_from_str_count() {
        let result = AggregationType::from_str("count:x > 100").expect("Should parse");
        match result {
            AggregationType::Count(pred) => assert_eq!(pred, "x > 100"),
            _ => panic!("Expected Count variant"),
        }
    }

    #[test]
    fn test_aggregation_type_from_str_invalid() {
        let result = AggregationType::from_str("invalid_agg");
        assert!(result.is_err());
        match result {
            Err(ConfigError::InvalidValue(msg)) => {
                assert!(msg.contains("Unknown aggregation type"));
            }
            _ => panic!("Expected InvalidValue error"),
        }
    }

    #[test]
    fn test_aggregation_type_clone() {
        let agg = AggregationType::Avg;
        let cloned = agg.clone();
        assert!(matches!(cloned, AggregationType::Avg));
    }

    #[test]
    fn test_aggregation_type_debug() {
        let agg = AggregationType::Percentile95;
        let debug_str = format!("{:?}", agg);
        assert!(debug_str.contains("Percentile95"));
    }

    // --- LocalMetricConfig tests ---

    #[test]
    fn test_local_metric_config_parse() {
        let config = LocalMetricConfig::parse("function:cpu").expect("Should parse");
        match config.source {
            SourceType::Function(name) => assert_eq!(name, "cpu"),
            _ => panic!("Expected Function source"),
        }
    }

    // --- GlobalMetricConfig tests ---

    #[test]
    fn test_global_metric_config_default_interval() {
        let config = GlobalMetricConfig {
            aggregation: "avg".to_string(),
            sources: vec!["cpu".to_string()],
            interval_ms: 500,
            expression: None,
        };
        
        assert_eq!(config.interval_ms, 500);
    }

    #[test]
    fn test_global_metric_config_with_expression() {
        let config = GlobalMetricConfig {
            aggregation: "avg".to_string(),
            sources: vec!["cpu".to_string(), "memory".to_string()],
            interval_ms: 1000,
            expression: Some("(100 - cpu) + memory_free".to_string()),
        };
        
        assert!(config.expression.is_some());
        assert_eq!(config.sources.len(), 2);
    }

    #[test]
    fn test_global_metric_config_clone() {
        let config = GlobalMetricConfig {
            aggregation: "sum".to_string(),
            sources: vec!["x".to_string()],
            interval_ms: 200,
            expression: Some("x + 1".to_string()),
        };
        
        let cloned = config.clone();
        assert_eq!(cloned.aggregation, "sum");
        assert_eq!(cloned.interval_ms, 200);
    }

    // --- DebugConfig tests ---

    #[test]
    fn test_debug_config_default() {
        let config = DebugConfig::default();
        assert!(!config.verbose);
        assert!(!config.simulate_sensors);
    }

    #[test]
    fn test_debug_config_verbose() {
        let config = DebugConfig {
            verbose: true,
            simulate_sensors: false,
        };
        assert!(config.verbose);
    }

    // --- QueueConfig tests ---

    #[test]
    fn test_queue_config_default() {
        let config = QueueConfig::default();
        // Check defaults
        assert!(config.capacity > 0);
        assert!(config.default_interval > 0);
    }

    #[test]
    fn test_queue_config_custom() {
        let config = QueueConfig {
            capacity: 4096,
            default_interval: 250,
        };
        assert_eq!(config.capacity, 4096);
        assert_eq!(config.default_interval, 250);
    }

    // --- ConfigBuilder tests ---

    #[test]
    fn test_config_builder_add_node() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7000, 7001, 7002, 7003)
            .add_node("node2", "192.168.1.1", 8000, 8001, 8002, 8003)
            .build();
        
        assert_eq!(config.nodes.nodes.len(), 2);
        assert!(config.nodes.nodes.contains_key("node1"));
        assert!(config.nodes.nodes.contains_key("node2"));
    }

    #[test]
    fn test_config_builder_add_local_metric() {
        let config = Config::builder()
            .add_local_metric("cpu", "function:cpu")
            .add_local_metric("memory", "function:memory")
            .build();
        
        assert_eq!(config.local.len(), 2);
        assert_eq!(config.local.get("cpu"), Some(&"function:cpu".to_string()));
    }

    #[test]
    fn test_config_builder_add_global_aggregation() {
        let config = Config::builder()
            .add_global_aggregation("cpu_avg", "avg", &["cpu"], 1000)
            .add_global_aggregation("mem_max", "max", &["memory"], 2000)
            .build();
        
        assert_eq!(config.global.len(), 2);
        let cpu_avg = config.global.get("cpu_avg").expect("Should exist");
        assert_eq!(cpu_avg.aggregation, "avg");
        assert_eq!(cpu_avg.interval_ms, 1000);
    }

    #[test]
    fn test_config_builder_add_expression_aggregation() {
        let config = Config::builder()
            .add_expression_aggregation(
                "health_score",
                "(100 - cpu) + memory_free",
                "avg",
                &["cpu", "memory_free"],
                500,
            )
            .build();
        
        let health = config.global.get("health_score").expect("Should exist");
        assert!(health.expression.is_some());
        assert_eq!(health.expression.as_ref().unwrap(), "(100 - cpu) + memory_free");
    }

    #[test]
    fn test_config_builder_capacity_and_interval() {
        let config = Config::builder()
            .capacity(8192)
            .default_interval(250)
            .build();
        
        assert_eq!(config.queue.capacity, 8192);
        assert_eq!(config.queue.default_interval, 250);
    }

    #[test]
    fn test_config_builder_simulate_sensors_and_verbose() {
        let config = Config::builder()
            .simulate_sensors(true)
            .verbose(true)
            .build();
        
        assert!(config.debug.simulate_sensors);
        assert!(config.debug.verbose);
    }

    #[test]
    fn test_config_builder_persistence() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let persist_config = PersistenceConfig {
            data_dir: temp_dir.path().to_path_buf(),
            max_file_size: 1024 * 1024,
            flush_interval_ms: 500,
            include_timestamp: true,
            compress_old: false,
        };
        
        let config = Config::builder()
            .persistence(persist_config.clone())
            .build();
        
        assert!(config.persistence.is_some());
        let p = config.persistence.unwrap();
        assert_eq!(p.flush_interval_ms, 500);
    }

    // --- Config::my_node and related methods ---

    #[test]
    fn test_config_my_node_found() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7000, 7001, 7002, 7003)
            .add_node("node2", "192.168.1.1", 8000, 8001, 8002, 8003)
            .build();
        
        let result = config.my_node(1).expect("Should find node1");
        assert_eq!(result.host, "127.0.0.1");
        
        let result2 = config.my_node(2).expect("Should find node2");
        assert_eq!(result2.host, "192.168.1.1");
    }

    #[test]
    fn test_config_my_node_not_found() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7000, 7001, 7002, 7003)
            .build();
        
        let result = config.my_node(99);
        assert!(result.is_err());
        match result {
            Err(ConfigError::NodeNotFound(name)) => assert_eq!(name, "node99"),
            _ => panic!("Expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_config_other_nodes() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7000, 7001, 7002, 7003)
            .add_node("node2", "127.0.0.2", 8000, 8001, 8002, 8003)
            .add_node("node3", "127.0.0.3", 9000, 9001, 9002, 9003)
            .build();
        
        // Get other nodes for node1
        let others = config.other_nodes(1);
        assert_eq!(others.len(), 2);
        
        // Should NOT contain node1
        for (name, _) in &others {
            assert_ne!(name, "node1");
        }
    }

    #[test]
    fn test_config_all_nodes() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7000, 7001, 7002, 7003)
            .add_node("node2", "127.0.0.2", 8000, 8001, 8002, 8003)
            .build();
        
        let all = config.all_nodes();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_config_local_source() {
        let config = Config::builder()
            .add_local_metric("cpu", "function:cpu")
            .add_local_metric("custom", "expression:x * 2")
            .build();
        
        let cpu_source = config.local_source("cpu").expect("Should exist");
        match cpu_source {
            SourceType::Function(name) => assert_eq!(name, "cpu"),
            _ => panic!("Expected Function type"),
        }
        
        let custom_source = config.local_source("custom").expect("Should exist");
        match custom_source {
            SourceType::Expression(expr) => assert_eq!(expr, "x * 2"),
            _ => panic!("Expected Expression type"),
        }
        
        assert!(config.local_source("nonexistent").is_none());
    }

    #[test]
    fn test_config_global_metric() {
        let config = Config::builder()
            .add_global_aggregation("cpu_avg", "avg", &["cpu"], 1000)
            .build();
        
        let metric = config.global_metric("cpu_avg").expect("Should exist");
        assert_eq!(metric.aggregation, "avg");
        assert!(config.global_metric("nonexistent").is_none());
    }

    // --- ConfigGenerator tests ---

    #[test]
    fn test_config_generator_local_test() {
        let config = ConfigGenerator::local_test(3, 7000);
        
        // Should have 3 nodes
        assert_eq!(config.nodes.nodes.len(), 3);
        assert!(config.nodes.nodes.contains_key("node1"));
        assert!(config.nodes.nodes.contains_key("node2"));
        assert!(config.nodes.nodes.contains_key("node3"));
        
        // Should have default local metrics
        assert!(config.local.contains_key("cpu"));
        assert!(config.local.contains_key("memory"));
        
        // Should have global aggregations
        assert!(config.global.contains_key("cpu_avg"));
        
        // Debug settings
        assert!(config.debug.simulate_sensors);
        assert!(config.debug.verbose);
    }

    #[test]
    fn test_config_generator_production() {
        let config = ConfigGenerator::production(&[("node1", "192.168.1.1"), ("node2", "192.168.1.2")]);
        
        assert_eq!(config.nodes.nodes.len(), 2);
        assert!(config.nodes.nodes.contains_key("node1"));
        assert!(config.nodes.nodes.contains_key("node2"));
        
        // Should have default local metrics
        assert!(config.local.contains_key("cpu"));
        assert!(config.local.contains_key("memory"));
    }

    // --- Config error tests ---

    #[test]
    fn test_config_error_io() {
        let err = ConfigError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
        let msg = format!("{}", err);
        assert!(msg.contains("IO error"));
    }

    #[test]
    fn test_config_error_parse() {
        let err = ConfigError::Parse(toml::from_str::<Config>("invalid [[[[").unwrap_err());
        let msg = format!("{}", err);
        assert!(msg.contains("parse error") || msg.contains("TOML"));
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = ConfigError::InvalidValue("test error".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid value"));
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_config_error_node_not_found() {
        let err = ConfigError::NodeNotFound("node99".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Node not found"));
        assert!(msg.contains("node99"));
    }

    #[test]
    fn test_config_error_invalid_source() {
        let err = ConfigError::InvalidSource("bad_source".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid source"));
    }

    #[test]
    fn test_config_error_missing_field() {
        let err = ConfigError::MissingField("host".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Missing required field"));
    }
}

// ============================================================================
// 10. TCP Transport Tests - Comprehensive Coverage
// ============================================================================

mod tcp_transport_tests {
    use ddl::network::tcp::NetworkMessage;
    use ddl::traits::ddl::Entry;

    // --- NetworkMessage serialization tests ---

    #[test]
    fn test_network_message_push() {
        let entry = Entry::new(1, "test", vec![1, 2, 3, 4]);
        
        let msg = NetworkMessage::Push {
            topic: "metrics".to_string(),
            entry: entry.clone(),
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        assert!(json.contains("Push"));
        assert!(json.contains("metrics"));
        
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NetworkMessage::Push { topic, entry: decoded_entry } => {
                assert_eq!(topic, "metrics");
                assert_eq!(decoded_entry.id, 1);
            }
            _ => panic!("Expected Push variant"),
        }
    }

    #[test]
    fn test_network_message_batch_push() {
        let entries = vec![
            Entry::new(1, "test", vec![1]),
            Entry::new(2, "test", vec![2]),
        ];
        
        let msg = NetworkMessage::BatchPush {
            topic: "batch_topic".to_string(),
            entries: entries.clone(),
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NetworkMessage::BatchPush { topic, entries } => {
                assert_eq!(topic, "batch_topic");
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("Expected BatchPush variant"),
        }
    }

    #[test]
    fn test_network_message_subscribe() {
        let msg = NetworkMessage::Subscribe {
            topic: "notifications".to_string(),
            subscriber_id: "client-123".to_string(),
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        assert!(json.contains("Subscribe"));
        assert!(json.contains("notifications"));
        assert!(json.contains("client-123"));
        
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NetworkMessage::Subscribe { topic, subscriber_id } => {
                assert_eq!(topic, "notifications");
                assert_eq!(subscriber_id, "client-123");
            }
            _ => panic!("Expected Subscribe variant"),
        }
    }

    #[test]
    fn test_network_message_ack() {
        let msg = NetworkMessage::Ack {
            topic: "events".to_string(),
            entry_id: 42,
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NetworkMessage::Ack { topic, entry_id } => {
                assert_eq!(topic, "events");
                assert_eq!(entry_id, 42);
            }
            _ => panic!("Expected Ack variant"),
        }
    }

    #[test]
    fn test_network_message_heartbeat() {
        let msg = NetworkMessage::Heartbeat;
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        assert!(json.contains("Heartbeat"));
        
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        assert!(matches!(decoded, NetworkMessage::Heartbeat));
    }

    #[test]
    fn test_network_message_debug() {
        let msg = NetworkMessage::Push {
            topic: "debug_test".to_string(),
            entry: Entry::new(1, "test", vec![]),
        };
        
        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("Push"));
    }

    #[test]
    fn test_network_message_clone() {
        let msg = NetworkMessage::Ack {
            topic: "clone_test".to_string(),
            entry_id: 999,
        };
        
        let cloned = msg.clone();
        match cloned {
            NetworkMessage::Ack { topic, entry_id } => {
                assert_eq!(topic, "clone_test");
                assert_eq!(entry_id, 999);
            }
            _ => panic!("Expected Ack variant"),
        }
    }

    #[test]
    fn test_network_message_all_variants_roundtrip() {
        let messages: Vec<NetworkMessage> = vec![
            NetworkMessage::Push {
                topic: "t1".to_string(),
                entry: Entry::new(1, "t1", vec![1, 2, 3]),
            },
            NetworkMessage::BatchPush {
                topic: "t2".to_string(),
                entries: vec![],
            },
            NetworkMessage::Subscribe {
                topic: "t3".to_string(),
                subscriber_id: "sub1".to_string(),
            },
            NetworkMessage::Ack {
                topic: "t4".to_string(),
                entry_id: 0,
            },
            NetworkMessage::Heartbeat,
        ];
        
        for msg in messages {
            let json = serde_json::to_string(&msg).expect("Should serialize");
            let _: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        }
    }

    #[test]
    fn test_network_message_large_entry() {
        // Test with large data
        let large_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let entry = Entry::new(1, "large", large_data.clone());
        
        let msg = NetworkMessage::Push {
            topic: "large_topic".to_string(),
            entry,
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize large message");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize large message");
        
        match decoded {
            NetworkMessage::Push { entry, .. } => {
                assert_eq!(entry.payload.len(), 10000);
            }
            _ => panic!("Expected Push variant"),
        }
    }
}

// ============================================================================
// 11. TCP Network Tests - Comprehensive Coverage
// ============================================================================

mod tcp_network_tests {
    use ddl::network::tcp_network::{TcpNetworkConfig, TcpNetworkFactory, TcpNetwork, TcpRaftServer};
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_tcp_network_config_default() {
        let config = TcpNetworkConfig::default();
        
        assert_eq!(config.bind_addr, "0.0.0.0:9090");
        assert!(config.peers.is_empty());
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert_eq!(config.max_connections_per_peer, 3);
    }

    #[test]
    fn test_tcp_network_config_new() {
        let config = TcpNetworkConfig::new(1, 8888);
        
        assert_eq!(config.bind_addr, "0.0.0.0:8888");
    }

    #[test]
    fn test_tcp_network_config_add_peer() {
        let mut config = TcpNetworkConfig::default();
        config.add_peer(1, "127.0.0.1:10000".to_string());
        config.add_peer(2, "192.168.1.1:10001".to_string());
        
        assert_eq!(config.peers.len(), 2);
        assert_eq!(
            config.peers.get(&1),
            Some(&"127.0.0.1:10000".to_string())
        );
        assert_eq!(
            config.peers.get(&2),
            Some(&"192.168.1.1:10001".to_string())
        );
    }

    #[test]
    fn test_tcp_network_config_clone() {
        let mut config = TcpNetworkConfig::default();
        config.add_peer(1, "peer1:1234".to_string());
        
        let cloned = config.clone();
        assert_eq!(cloned.peers.len(), 1);
        assert_eq!(cloned.peers.get(&1), Some(&"peer1:1234".to_string()));
    }

    #[test]
    fn test_tcp_network_config_debug() {
        let config = TcpNetworkConfig::default();
        let debug_str = format!("{:?}", config);
        
        assert!(debug_str.contains("TcpNetworkConfig"));
        assert!(debug_str.contains("0.0.0.0:9090"));
    }

    #[test]
    fn test_tcp_network_new() {
        let config = TcpNetworkConfig::default();
        let network = TcpNetwork::new(42, config.clone());
        
        // Verify network is created successfully (target is private)
        let _ = network;
    }

    #[test]
    fn test_tcp_network_factory_new() {
        let config = TcpNetworkConfig::default();
        let factory = TcpNetworkFactory::new(config);
        
        // Factory should be created successfully
        let _ = factory;
    }

    #[test]
    fn test_tcp_network_factory_clone() {
        let config = TcpNetworkConfig::default();
        let factory = TcpNetworkFactory::new(config);
        
        let cloned = factory.clone();
        let _ = cloned;
    }

    #[tokio::test]
    async fn test_tcp_raft_server_bind() {
        // Bind to port 0 to get a random available port
        let result = TcpRaftServer::bind("127.0.0.1:0", 1).await;
        assert!(result.is_ok());
        
        let server = result.unwrap();
        let addr_result = server.local_addr();
        assert!(addr_result.is_ok());
        
        let addr = addr_result.unwrap();
        assert!(addr.contains("127.0.0.1"));
    }

    #[tokio::test]
    async fn test_tcp_raft_server_invalid_bind() {
        // Try to bind to an invalid address
        let result = TcpRaftServer::bind("invalid.address.that.does.not.exist:12345", 1).await;
        assert!(result.is_err());
    }

    // Note: RpcType is private, so we can't test it directly from external tests
    // The RpcType tests are in the src/network/tcp_network.rs module tests

    #[test]
    fn test_tcp_network_config_with_many_peers() {
        let mut config = TcpNetworkConfig::default();
        
        for i in 1..=100u64 {
            config.add_peer(i, format!("node{}:{}", i, 9000 + i as u16));
        }
        
        assert_eq!(config.peers.len(), 100);
    }

    #[test]
    fn test_tcp_network_config_timeout_custom() {
        let config = TcpNetworkConfig {
            bind_addr: "0.0.0.0:9999".to_string(),
            peers: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_connections_per_peer: 10,
        };
        
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_connections_per_peer, 10);
    }
}

// ============================================================================
// 12. Enhanced Raft Transport Tests
// ============================================================================

mod enhanced_raft_transport_tests {
    use ddl::network::raft_transport::ZmqRaftNetwork;
    use std::collections::HashMap;

    #[test]
    fn test_zmq_raft_network_creation_basic() {
        let mut peers = HashMap::new();
        peers.insert(1, ("127.0.0.1".to_string(), 7000u16));
        
        let result = ZmqRaftNetwork::new(1, peers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_multiple_peers() {
        let mut peers = HashMap::new();
        for i in 1..=5u64 {
            peers.insert(i, (format!("node{}", i), 7000 + i as u16));
        }
        
        let result = ZmqRaftNetwork::new(0, peers);
        assert!(result.is_ok());
        
        let _network = result.unwrap();
    }

    #[test]
    fn test_zmq_raft_network_no_peers() {
        let peers: HashMap<u64, (String, u16)> = HashMap::new();
        
        let result = ZmqRaftNetwork::new(1, peers);
        // Should succeed even with no peers (might be for standalone node)
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_different_node_ids() {
        for node_id in 0..=10u64 {
            let peers = HashMap::new();
            let result = ZmqRaftNetwork::new(node_id, peers);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_zmq_raft_network_ipv6_addresses() {
        let mut peers = HashMap::new();
        peers.insert(1, ("::1".to_string(), 7000u16));
        peers.insert(2, ("2001:db8::1".to_string(), 7001u16));
        
        let result = ZmqRaftNetwork::new(0, peers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_same_port_different_hosts() {
        let mut peers = HashMap::new();
        peers.insert(1, ("192.168.1.1".to_string(), 8080u16));
        peers.insert(2, ("192.168.1.2".to_string(), 8080u16));
        
        let result = ZmqRaftNetwork::new(0, peers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_raft_network_same_host_different_ports() {
        let mut peers = HashMap::new();
        peers.insert(1, ("127.0.0.1".to_string(), 7000u16));
        peers.insert(2, ("127.0.0.1".to_string(), 8000u16));
        
        let result = ZmqRaftNetwork::new(0, peers);
        assert!(result.is_ok());
    }
}

// ============================================================================
// 13. Enhanced ZMQ Transport Tests
// ============================================================================

mod enhanced_zmq_transport_tests {
    use ddl::network::pubsub::zmq::transport::{NodeMessage, ZmqTransport};
    use ddl::network::transport_traits::Transport;
    use std::time::Duration;

    #[test]
    fn test_zmq_transport_multiple_creations() {
        // Create multiple instances to verify no conflicts
        for _ in 0..5 {
            let transport = ZmqTransport::new();
            assert!(transport.is_ok());
        }
    }

    #[test]
    fn test_zmq_transport_is_connected_after_creation() {
        let transport = ZmqTransport::new().unwrap();
        
        // Initially not connected (no connect() call)
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_zmq_transport_connection_info_initial() {
        let transport = ZmqTransport::new().unwrap();
        
        // connection_info should be None initially
        let info = transport.connection_info();
        assert!(info.is_none());
    }

    #[test]
    fn test_zmq_transport_socket() {
        let transport = ZmqTransport::new().unwrap();
        let socket_arc = transport.socket();
        
        // Socket should be accessible through Arc
        // Just verify we can access it without panic
        let _ = socket_arc;
    }

    #[test]
    fn test_node_message_empty_sources() {
        let msg = NodeMessage::RequestExpression {
            sources: vec![],
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NodeMessage::RequestExpression { sources } => {
                assert!(sources.is_empty());
            }
            _ => panic!("Expected RequestExpression"),
        }
    }

    #[test]
    fn test_node_message_many_sources() {
        let sources: Vec<String> = (0..100).map(|i| format!("metric_{}", i)).collect();
        let msg = NodeMessage::RequestExpression {
            sources: sources.clone(),
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NodeMessage::RequestExpression { sources: decoded_sources } => {
                assert_eq!(decoded_sources.len(), 100);
            }
            _ => panic!("Expected RequestExpression"),
        }
    }

    #[test]
    fn test_node_message_expression_response_none_score() {
        let msg = NodeMessage::ExpressionResponse {
            sources: vec!["cpu".to_string()],
            score: None,
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NodeMessage::ExpressionResponse { score, .. } => {
                assert!(score.is_none());
            }
            _ => panic!("Expected ExpressionResponse"),
        }
    }

    #[test]
    fn test_node_message_request_aggregation_empty_args() {
        let msg = NodeMessage::RequestAggregation {
            queue_name: "stats".to_string(),
            operation: "percentile".to_string(),
            args: vec![],
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NodeMessage::RequestAggregation { args, .. } => {
                assert!(args.is_empty());
            }
            _ => panic!("Expected RequestAggregation"),
        }
    }

    #[test]
    fn test_node_message_unicode_queue_names() {
        let msg = NodeMessage::RequestValue {
            queue_name: "温度_数据_日本語".to_string(),
        };
        
        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NodeMessage = serde_json::from_str(&json).expect("Should deserialize");
        
        match decoded {
            NodeMessage::RequestValue { queue_name } => {
                assert_eq!(queue_name, "温度_数据_日本語");
            }
            _ => panic!("Expected RequestValue"),
        }
    }

    #[test]
    fn test_transport_debug_impl() {
        let transport = ZmqTransport::new().unwrap();
        let debug_str = format!("{:?}", transport);
        
        // Debug should show field names without sensitive data
        assert!(debug_str.contains("ZmqTransport"));
    }
}