#[cfg(test)]
mod tests {
    use ddl::config::{Config, ConfigGenerator, NodeConfig, NodeTable};
    use std::collections::HashMap;

    #[test]
    fn test_source_type_parse() {
        let func = ddl::config::SourceType::parse("function:cpu").unwrap();
        match func {
            ddl::config::SourceType::Function(name) => assert_eq!(name, "cpu"),
            _ => panic!("Expected Function variant"),
        }

        let expr = ddl::config::SourceType::parse("expression:x * 2").unwrap();
        match expr {
            ddl::config::SourceType::Expression(e) => assert_eq!(e, "x * 2"),
            _ => panic!("Expected Expression variant"),
        }
    }

    #[test]
    fn test_source_type_variables() {
        let func = ddl::config::SourceType::parse("function:cpu").unwrap();
        assert_eq!(func.variables(), vec!["cpu"]);

        let expr = ddl::config::SourceType::parse("expression:cpu + memory").unwrap();
        let vars = expr.variables();
        assert!(vars.contains(&"cpu".to_string()));
        assert!(vars.contains(&"memory".to_string()));
    }

    #[test]
    fn test_local_test_config() {
        let config = ConfigGenerator::local_test(3, 7000);
        assert_eq!(config.all_nodes().len(), 3);
        assert!(config.my_node(1).is_ok());
        assert!(config.my_node(4).is_err()); // Should not exist
    }

    #[test]
    fn test_production_config() {
        let nodes = vec![("node1", "192.168.1.10"), ("node2", "192.168.1.11")];
        let config = ConfigGenerator::production(&nodes);
        assert_eq!(config.all_nodes().len(), 2);
        assert!(config.my_node(1).is_ok());
        assert!(config.my_node(3).is_err()); // Should not exist
    }

    #[test]
    fn test_config_builder() {
        let config = Config::builder()
            .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
            .add_node("node2", "127.0.0.1", 7101, 7102, 7103, 7104)
            .add_node("node10", "127.0.0.1", 8001, 8002, 8003, 8004) // This would have failed before
            .add_local_metric("cpu", "function:cpu")
            .add_global_aggregation("cpu_avg", "avg", &["cpu"], 1000)
            .build();

        assert_eq!(config.all_nodes().len(), 3); // Now we have 3 nodes
        assert_eq!(config.local.len(), 1);
        assert_eq!(config.global.len(), 1);
        assert!(config.my_node(1).is_ok());
        assert!(config.my_node(10).is_ok()); // Now this should work
        assert!(config.my_node(3).is_err()); // Should not exist
    }

    #[test]
    fn test_collect_nodes() {
        let mut table = NodeTable::default();
        table.nodes.insert(
            "node1".to_string(),
            NodeConfig {
                host: "127.0.0.1".to_string(),
                communication_port: 7001,
                query_port: 7002,
                coordination_port: 7003,
                pubsub_port: 7004,
            },
        );
        table.nodes.insert(
            "node2".to_string(),
            NodeConfig {
                host: "127.0.0.1".to_string(),
                communication_port: 7101,
                query_port: 7102,
                coordination_port: 7103,
                pubsub_port: 7104,
            },
        );

        let nodes = table.collect_nodes();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.iter().any(|(name, _)| name == "node1"));
        assert!(nodes.iter().any(|(name, _)| name == "node2"));
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
            [node1]
            host = "127.0.0.1"
            communication_port = 7067
            query_port = 7069
            coordination_port = 7070
            pubsub_port = 7071

            [node2]
            host = "127.0.0.1"
            communication_port = 7167
            query_port = 7169
            coordination_port = 7170
            pubsub_port = 7171

            [local]
            cpu = "function:cpu"
            memory = "function:memory"

            [global.cpu_avg]
            aggregation = "avg"
            sources = ["cpu"]
            interval_ms = 1000
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.all_nodes().len(), 2);
        assert_eq!(config.local.len(), 2);
        assert_eq!(config.global.len(), 1);
        assert!(config.my_node(1).is_ok());
        assert!(config.my_node(2).is_ok());
        assert!(config.my_node(3).is_err());
    }
}
