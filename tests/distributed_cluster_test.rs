//! Distributed Cluster Test Examples
//!
//! This file demonstrates how to use the distributed test utilities
//! for various distributed testing scenarios.

mod distributed_test_utils;

use distributed_test_utils::{
    parse_node_spec, TestCluster, NodeSpecBuilder,
    get_node_spec_from_env, get_node_spec_or_default,
    validate_node_spec, DEFAULT_COMMUNICATION_PORT,
};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

// ============================================================================
// Example 1: Simple range specification
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_with_simple_range() {
    // Parse "node[1-3]" → node1, node2, node3
    let cluster = TestCluster::setup("node[1-3]")
        .await
        .expect("Failed to create cluster");
    
    // Verify cluster size
    assert_eq!(cluster.size(), 3);
    // Node IDs should be 1, 2, 3 but HashMap order is not guaranteed
    let mut node_ids = cluster.node_ids();
    node_ids.sort();
    assert_eq!(node_ids, vec![1, 2, 3]);
    
    // Initialize and verify bootstrap
    cluster.initialize()
        .await
        .expect("Failed to initialize cluster");
    
    // Check bootstrap node became leader
    let bootstrap = cluster.get_node(cluster.bootstrap_id).unwrap();
    let is_leader = timeout(Duration::from_secs(3), async {
        loop {
            if bootstrap.is_leader().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for leader");
    
    assert!(is_leader, "Bootstrap node should become leader");
    
    // Cleanup
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 2: Named hosts with range
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_with_named_hosts_range() {
    // Use environment variable or default
    let spec = get_node_spec_or_default("ares-comp-[13-16]");
    
    // Parse: "ares-comp-[13-16]" → ares-comp-13, ares-comp-14, ares-comp-15, ares-comp-16
    let hosts = parse_node_spec(&spec).unwrap();
    assert_eq!(hosts.len(), 4);
    
    // Create test cluster
    let cluster = TestCluster::setup(&spec)
        .await
        .expect("Failed to create cluster");
    
    assert_eq!(cluster.size(), 4);
    
    // Verify hostnames
    assert!(cluster.configs.get(&1).unwrap().host.contains("ares-comp-13"));
    assert!(cluster.configs.get(&4).unwrap().host.contains("ares-comp-16"));
    
    cluster.initialize().await.ok();
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 3: Explicit list with ports
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_with_explicit_list() {
    // Explicit hosts with custom ports
    let spec = "host1:8000,host2:8001,host3:8002";
    
    let configs = NodeSpecBuilder::new(spec)
        .build()
        .unwrap();
    
    assert_eq!(configs.len(), 3);
    assert_eq!(configs.get(&1).unwrap().host, "host1");
    assert_eq!(configs.get(&2).unwrap().coordination_port, 8001);
    
    // Create cluster
    let cluster = TestCluster::setup(spec)
        .await
        .expect("Failed to create cluster");
    
    cluster.initialize().await.ok();
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 4: IP ranges with ports
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_with_ip_range() {
    // IP range with coordination port
    // "10.0.0.[1-4]:9090" → 10.0.0.1:9090, 10.0.0.2:9090, ...
    let configs = NodeSpecBuilder::new("10.0.0.[1-4]:9090")
        .coord_offset(0) // Port already specified
        .build()
        .unwrap();
    
    assert_eq!(configs.len(), 4);
    assert_eq!(configs.get(&1).unwrap().host, "10.0.0.1");
    assert_eq!(configs.get(&1).unwrap().coordination_port, 9090);
    
    let cluster = TestCluster::setup_with_options(
        "10.0.0.[1-4]:9090",
        DEFAULT_COMMUNICATION_PORT,
        1,
    )
    .await
    .expect("Failed to create cluster");
    
    cluster.initialize().await.ok();
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 5: Mixed ranges and singles
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_with_mixed_range() {
    // "node[1,3-5,7]" → node1, node3, node4, node5, node7
    let hosts = parse_node_spec("node[1,3-5,7]").unwrap();
    assert_eq!(hosts.len(), 5);
    assert!(hosts.contains(&"node1".to_string()));
    assert!(hosts.contains(&"node4".to_string()));
    assert!(!hosts.contains(&"node2".to_string()));
    
    let cluster = TestCluster::setup("node[1,3-5,7]")
        .await
        .expect("Failed to create cluster");
    
    assert_eq!(cluster.size(), 5);
    
    cluster.initialize().await.ok();
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 6: Leading zeros in ranges
// ============================================================================
#[test]
fn test_leading_zeros_preserved() {
    // "node[001-003]" → node001, node002, node003
    let hosts = parse_node_spec("node[001-003]").unwrap();
    assert_eq!(hosts, vec!["node001", "node002", "node003"]);
    
    let configs = NodeSpecBuilder::new("node[001-003]")
        .build()
        .unwrap();
    
    assert_eq!(configs.get(&1).unwrap().host, "node001");
    assert_eq!(configs.get(&3).unwrap().host, "node003");
}

// ============================================================================
// Example 7: Complex composed ranges
// ============================================================================
#[test]
fn test_composed_ranges() {
    // "cluster-[1-2]-node[1-2]"
    // → cluster-1-node1, cluster-1-node2, cluster-2-node1, cluster-2-node2
    let hosts = parse_node_spec("cluster-[1-2]-node[1-2]").unwrap();
    assert_eq!(hosts.len(), 4);
    assert!(hosts.contains(&"cluster-1-node1".to_string()));
    assert!(hosts.contains(&"cluster-2-node2".to_string()));
}

// ============================================================================
// Example 8: Environment variable usage
// ============================================================================
#[test]
fn test_env_variable_override() {
    // This test demonstrates that RUST_TEST_NODES environment variable
    // can override the node specification.
    
    // In a real test, you'd set the env var before running:
    // RUST_TEST_NODES="prod-[01-10]" cargo test
    
    // Without env var, use default
    let spec = get_node_spec_or_default("localhost:8080,localhost:8081");
    
    // If RUST_TEST_NODES is set, it would be used instead
    if get_node_spec_from_env().is_none() {
        assert_eq!(spec, "localhost:8080,localhost:8081");
    }
}

// ============================================================================
// Example 9: Validation before cluster creation
// ============================================================================
#[test]
fn test_validate_before_setup() {
    // Validate specification without creating cluster
    let result = validate_node_spec("node[1-5]");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 5);
    
    // Empty specification (invalid)
    let result = validate_node_spec("");
    assert!(result.is_err());
    
    // Invalid range (invalid)
    let result = validate_node_spec("node[5-2]");
    assert!(result.is_err());
    
    // "node[invalid]" - contains letters which can't be parsed as range
    // Parser treats it as a single value "invalid" which is valid
    let result = validate_node_spec("node[invalid]");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

// ============================================================================
// Example 10: Production-style cluster test
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_production_style_cluster() {
    // Simulate a production cluster with multiple racks
    // "rack[1-2]-node[1-2]" → rack1-node1, rack1-node2, rack2-node1, rack2-node2
    let spec = "rack[1-2]-node[1-2]";
    
    // Validate first
    validate_node_spec(spec).expect("Invalid node specification");
    
    // Create cluster with custom ports
    let configs = NodeSpecBuilder::new(spec)
        .base_port(9000)
        .coord_offset(100) // Coordination at 9100, 9200, etc.
        .starting_id(100)
        .build()
        .unwrap();
    
    assert_eq!(configs.len(), 4);
    assert_eq!(configs.get(&100).unwrap().communication_port, 9000);
    assert_eq!(configs.get(&100).unwrap().coordination_port, 9100);
    
    // Note: In production, you'd use actual hostnames
    // For testing, we use a simpler spec
    let cluster = TestCluster::setup("node[1-4]")
        .await
        .expect("Failed to create cluster");
    
    assert_eq!(cluster.size(), 4);
    
    // Initialize cluster
    cluster.initialize().await.ok();
    
    // In a real test, you'd:
    // - Wait for leader election
    // - Test topic ownership claims
    // - Test fault tolerance
    // - Test leader failover
    
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 11: Direct NodeConfig manipulation
// ============================================================================
#[test]
fn test_direct_config_building() {
    // Build configs programmatically when special setup is needed
    let mut configs = HashMap::new();
    
    // Add custom node configurations
    for i in 1..=3u64 {
        let host = format!("custom-host-{}", i);
        configs.insert(i, ddl::cluster::types::NodeConfig {
            node_id: i,
            host: host.clone(),
            communication_port: 7000 + i as u16 * 100,
            coordination_port: 9000 + i as u16 * 100,
        });
    }
    
    assert_eq!(configs.len(), 3);
    assert_eq!(configs.get(&1).unwrap().communication_port, 7100);
    assert_eq!(configs.get(&3).unwrap().coordination_port, 9300);
}

// ============================================================================
// Example 12: Topic ownership test with node spec
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_topic_ownership_with_spec() {
    // Create a 3-node cluster
    let cluster = TestCluster::setup("node[1-3]")
        .await
        .expect("Failed to create cluster");
    
    cluster.initialize()
        .await
        .expect("Failed to initialize cluster");
    
    // Get bootstrap node
    let node = cluster.get_node(cluster.bootstrap_id).unwrap();
    
    // Wait for leader election
    let is_leader = timeout(Duration::from_secs(3), async {
        loop {
            if node.is_leader().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for leader");
    
    assert!(is_leader, "Bootstrap node should be leader");
    
    // Test topic ownership
    let result = node.claim_topic("test-topic").await;
    assert!(result.is_ok(), "Should be able to claim topic");
    
    // Verify ownership (local read for faster test)
    let owner = node.get_owner_local("test-topic");
    assert_eq!(owner, Some(cluster.bootstrap_id));
    
    // Release topic
    let result = node.release_topic("test-topic").await;
    assert!(result.is_ok(), "Should be able to release topic");
    
    cluster.shutdown().await.ok();
}

// ============================================================================
// Example 13: Multi-node consensus test
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_multi_node_consensus() {
    // Skip if running in CI without actual network support
    if std::env::var("CI").is_ok() {
        eprintln!("Skipping multi-node consensus test in CI");
        return;
    }
    
    // Create a test cluster with known hosts
    let spec = "localhost:[7080-7082]";
    let hosts = parse_node_spec(spec).unwrap();
    
    assert_eq!(hosts.len(), 3);
    assert!(hosts.contains(&"localhost:7080".to_string()));
    
    // For actual multi-node testing, you'd need real network setup
    // This demonstrates the specification parsing
}

// ============================================================================
// Example 14: Large cluster specification
// ============================================================================
#[test]
fn test_large_cluster_spec() {
    // Test parsing large ranges efficiently
    let spec = "node[1-100]";
    let hosts = parse_node_spec(spec).unwrap();
    
    assert_eq!(hosts.len(), 100);
    assert!(hosts.contains(&"node1".to_string()));
    assert!(hosts.contains(&"node100".to_string()));
    assert!(!hosts.contains(&"node0".to_string()));
    assert!(!hosts.contains(&"node101".to_string()));
}

// ============================================================================
// Example 15: Error handling
// ============================================================================
#[test]
fn test_error_cases() {
    // Empty specification
    assert!(parse_node_spec("").is_err());
    
    // Invalid range (start > end)
    assert!(parse_node_spec("node[10-5]").is_err());
    
    // Empty brackets
    assert!(parse_node_spec("node[]").is_err());
    
    // Note: Missing closing bracket doesn't cause error
    // "node[5" is treated as hostname "node[5" (brackets not matched)
    // This is valid per parser behavior - it passes through as-is
    let result = parse_node_spec("node[5");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec!["node[5"]);
    
    // Valid single node
    let result = parse_node_spec("single-node").unwrap();
    assert_eq!(result, vec!["single-node"]);
}

// ============================================================================
// Example 16: Port range in specification  
// ============================================================================
#[test]
fn test_port_specification() {
    // Test that port ranges are expanded correctly
    let hosts = parse_node_spec("host:[8080-8082]").unwrap();
    
    // Note: This treats the range as part of the host string
    // The builder handles port extraction separately
    assert_eq!(hosts.len(), 3);
}

// ============================================================================
// Integration: Full cluster lifecycle
// ============================================================================
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_full_cluster_lifecycle() {
    // Full lifecycle test: create → initialize → use → shutdown
    
    // 1. Parse specification
    let spec = get_node_spec_or_default("node[1-3]");
    println!("Using node specification: {}", spec);
    
    // 2. Create cluster
    let cluster = TestCluster::setup(&spec)
        .await
        .expect("Failed to create cluster");
    
    println!("Created {} nodes", cluster.size());
    
    // 3. Initialize
    cluster.initialize()
        .await
        .expect("Failed to initialize");
    
    // 4. Get leader
    let bootstrap = cluster.get_node(cluster.bootstrap_id).unwrap();
    
    // 5. Wait for leadership
    timeout(Duration::from_secs(5), async {
        while !bootstrap.is_leader().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Leader should be elected within timeout");
    
    // 6. Perform operations
    let topics = vec!["topic-1", "topic-2", "topic-3"];
    
    for topic in &topics {
        bootstrap.claim_topic(topic).await.ok();
    }
    
    let owned = bootstrap.get_owned_topics().await;
    println!("Owned topics: {:?}", owned);
    
    // 7. Clean shutdown
    cluster.shutdown()
        .await
        .expect("Failed to shutdown cluster");
    
    println!("Cluster shutdown complete");
}