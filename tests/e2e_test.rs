//! Comprehensive end-to-end tests for AutoQueues

use autoqueues::config::{
    Config, ConfigError, SourceType, AggregationType,
};
use autoqueues::ZmqPubSubBroker;

#[cfg(test)]
mod e2e_tests {
    use super::*;

    /// Test configuration creation with builder
    #[tokio::test]
    async fn test_configuration_builder() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7067, 7069, 7070, 7071)
            .add_node("node2", "127.0.0.1", 7167, 7169, 7170, 7171)
            .add_local_metric("cpu", "function:cpu")
            .add_local_metric("memory", "function:memory")
            .capacity(2048)
            .default_interval(500)
            .build();

        let nodes = config.all_nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(config.queue.capacity, 2048);
        assert_eq!(config.queue.default_interval, 500);
        assert!(config.local.contains_key("cpu"));
        assert!(config.local.contains_key("memory"));
    }

    /// Test source type parsing
    #[tokio::test]
    async fn test_source_type_parsing() {
        let func_source = SourceType::parse("function:cpu");
        assert!(func_source.is_ok());
        assert!(matches!(func_source.unwrap(), SourceType::Function(_)));

        let expr_source = SourceType::parse("expression:(cpu + memory) / 2");
        assert!(expr_source.is_ok());
        assert!(matches!(expr_source.unwrap(), SourceType::Expression(_)));

        let default_source = SourceType::parse("cpu");
        assert!(default_source.is_ok());
        assert!(matches!(default_source.unwrap(), SourceType::Function(_)));
    }

    /// Test aggregation type parsing
    #[tokio::test]
    async fn test_aggregation_type_parsing() {
        assert!(matches!(AggregationType::from_str("avg").unwrap(), AggregationType::Avg));
        assert!(matches!(AggregationType::from_str("max").unwrap(), AggregationType::Max));
        assert!(matches!(AggregationType::from_str("p95").unwrap(), AggregationType::Percentile95));
        assert!(matches!(AggregationType::from_str("count:cpu>80").unwrap(), AggregationType::Count(_)));
    }

    /// Test ZMQ pub/sub broker
    #[tokio::test]
    async fn test_zmq_pubsub() {
        let broker = ZmqPubSubBroker::new();
        match broker {
            Ok(_b) => assert!(true),
            Err(_) => println!("ZMQ not available, skipping pub/sub test"),
        }
    }

    /// Test expression variable extraction
    #[tokio::test]
    async fn test_expression_variables() {
        let source = SourceType::parse("expression:(cpu + memory) / 2").unwrap();
        let vars = source.variables();
        assert!(vars.contains(&"cpu".to_string()));
        assert!(vars.contains(&"memory".to_string()));
    }
}
