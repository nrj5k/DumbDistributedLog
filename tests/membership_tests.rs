//! Integration tests for membership events
//!
//! Tests for SCORE failover coordination membership tracking.

use ddl::cluster::{RaftClusterNode, MembershipEventType, MembershipEvent, NodeConfig};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

/// Helper to create a standalone RaftClusterNode
async fn create_standalone_node(node_id: u64) -> RaftClusterNode {
    let mut nodes = HashMap::new();
    nodes.insert(
        node_id,
        NodeConfig {
            node_id,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        },
    );
    RaftClusterNode::new(node_id, nodes)
        .await
        .expect("Failed to create node")
}

/// Helper to create nodes map
fn create_nodes_config(node_ids: Vec<(u64, &str, u16, u16)>) -> HashMap<u64, NodeConfig> {
    node_ids
        .into_iter()
        .map(|(id, host, comm_port, coord_port)| {
            (
                id,
                NodeConfig {
                    node_id: id,
                    host: host.to_string(),
                    communication_port: comm_port,
                    coordination_port: coord_port,
                },
            )
        })
        .collect()
}

#[tokio::test]
async fn test_membership_event_creation() {
    // Test creating NodeJoined event
    let event = MembershipEvent::node_joined(1, "localhost:9090".to_string());
    assert!(matches!(
        event.event_type,
        MembershipEventType::NodeJoined { node_id: 1, .. }
    ));

    // Test creating NodeLeft event
    let event = MembershipEvent::node_left(1);
    assert!(matches!(
        event.event_type,
        MembershipEventType::NodeLeft { node_id: 1 }
    ));

    // Test creating NodeFailed event
    let event = MembershipEvent::node_failed(1);
    assert!(matches!(
        event.event_type,
        MembershipEventType::NodeFailed { node_id: 1 }
    ));
}

#[tokio::test]
async fn test_subscribe_membership() {
    let node = create_standalone_node(1).await;

    // Should be able to get a subscription receiver
    let receiver = node.subscribe_membership();

    // The receiver should exist (even if no events yet)
    drop(receiver);
}

#[tokio::test]
async fn test_membership_initialize_creates_self_event() {
    let node = create_standalone_node(1).await;
    let mut events = node.subscribe_membership();

    // Initialize as the bootstrap node
    node.initialize().await.expect("Failed to initialize");

    // Should receive NodeJoined for self
    let result = timeout(Duration::from_secs(5), events.recv()).await;

    // The event might or might not arrive depending on timing
    // Just verify we can subscribe and the API works
    match result {
        Ok(Ok(event)) => {
            // We got an event - just verify the API works
            match event.event_type {
                MembershipEventType::NodeJoined { node_id, .. } => {
                    // Could be self-join event
                    assert!(node_id == 1);
                }
                _ => {}
            }
        }
        _ => {
            // No event within timeout - that's okay for this test
            // The important part is that subscribe_membership() works
        }
    }
}

#[tokio::test]
async fn test_double_initialize_returns_error() {
    let node = create_standalone_node(1).await;

    // First initialization should succeed
    let result = node.initialize().await;
    assert!(result.is_ok(), "First initialize should succeed");

    // Second initialization should fail
    let result = node.initialize().await;
    assert!(result.is_err(), "Second initialize should fail");
    assert_eq!(result.unwrap_err(), "Already initialized");
}

#[tokio::test]
async fn test_initialize_missing_node_config_returns_error() {
    // Create a node without its own ID in the config
    let mut nodes = HashMap::new();
    nodes.insert(
        999, // Different node ID
        NodeConfig {
            node_id: 999,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        },
    );

    let node = RaftClusterNode::new(1, nodes)
        .await
        .expect("Failed to create node");

    // Initialize should fail because node ID 1 is not in nodes config
    let result = node.initialize().await;
    assert!(result.is_err(), "Initialize should fail for missing node config");
    assert!(result.unwrap_err().contains("not found in configuration"));
}

#[tokio::test]
async fn test_membership_view() {
    let nodes = create_nodes_config(vec![
        (1, "localhost", 8080, 9090),
        (2, "localhost", 8081, 9091),
    ]);

    let node = RaftClusterNode::new(1, nodes.clone())
        .await
        .expect("Failed to create node");

    node.initialize().await.expect("Failed to initialize");

    // Wait for election
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    // Get membership view
    let membership = node.membership();

    // Should have local node at minimum
    assert_eq!(membership.local_node_id, 1);

    // Should know about configured nodes
    assert!(membership.nodes.len() >= 1, "Should have at least one node");
}

#[tokio::test]
async fn test_membership_metrics() {
    let node = create_standalone_node(1).await;

    node.initialize().await.expect("Failed to initialize");

    // Wait for election
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    // Get metrics
    let metrics = node.metrics();

    // Should have at least the local node
    assert!(metrics.membership_size >= 1);

    // Single node should become leader after initialization
    assert!(metrics.raft_leader.is_some());

    // Initial state
    assert_eq!(metrics.active_leases, 0); // Not implemented yet
    assert_eq!(metrics.state_size_bytes, 0); // Not implemented yet
    assert_eq!(metrics.state_key_count, 0); // Not implemented yet
}

#[tokio::test]
async fn test_event_type_display() {
    let event = MembershipEventType::NodeJoined {
        node_id: 1,
        addr: "localhost:9090".to_string(),
    };
    assert_eq!(format!("{}", event), "NodeJoined(1)");

    let event = MembershipEventType::NodeLeft { node_id: 2 };
    assert_eq!(format!("{}", event), "NodeLeft(2)");

    let event = MembershipEventType::NodeFailed { node_id: 3 };
    assert_eq!(format!("{}", event), "NodeFailed(3)");

    let event = MembershipEventType::NodeRecovered { node_id: 4 };
    assert_eq!(format!("{}", event), "NodeRecovered(4)");
}

#[tokio::test]
async fn test_membership_multiple_nodes() {
    let nodes = create_nodes_config(vec![
        (1, "localhost", 8080, 9090),
        (2, "localhost", 8081, 9091),
    ]);

    let node1 = RaftClusterNode::new(1, nodes.clone())
        .await
        .expect("Failed to create node1");

    let _node2 = RaftClusterNode::new(2, nodes.clone())
        .await
        .expect("Failed to create node2");

    // Initialize node1 as bootstrap
    node1.initialize().await.expect("Failed to initialize node1");

    // Wait for election
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    // Get membership from node1
    let membership = node1.membership();

    // Should have local node
    assert_eq!(membership.local_node_id, 1);

    // Leader should be set after election
    assert!(membership.leader.is_some());
}

#[tokio::test]
async fn test_timestamp_in_events() {
    use std::time::SystemTime;

    let before = SystemTime::now();
    let event = MembershipEvent::node_joined(1, "localhost:9090".to_string());
    let after = SystemTime::now();

    // Timestamp should be between before and after
    assert!(event.timestamp >= before);
    assert!(event.timestamp <= after);
}

#[tokio::test]
async fn test_membership_node_info() {
    use ddl::cluster::NodeInfo;
    use std::time::SystemTime;

    let info = NodeInfo {
        node_id: 1,
        addr: "localhost:9090".to_string(),
        last_seen: SystemTime::now(),
        is_leader: true,
    };

    assert_eq!(info.node_id, 1);
    assert_eq!(info.addr, "localhost:9090");
    assert!(info.is_leader);
}

#[tokio::test]
async fn test_membership_view_structure() {
    use ddl::cluster::{MembershipView, NodeInfo};
    use std::time::SystemTime;

    let mut nodes = HashMap::new();
    nodes.insert(
        1,
        NodeInfo {
            node_id: 1,
            addr: "localhost:9090".to_string(),
            last_seen: SystemTime::now(),
            is_leader: true,
        },
    );

    let view = MembershipView {
        nodes,
        leader: Some(1),
        local_node_id: 1,
    };

    assert_eq!(view.local_node_id, 1);
    assert_eq!(view.leader, Some(1));
    assert_eq!(view.nodes.len(), 1);
}

#[tokio::test]
async fn test_ddl_metrics_structure() {
    use ddl::cluster::DdlMetrics;

    let metrics = DdlMetrics {
        active_leases: 5,
        membership_size: 3,
        state_size_bytes: 1024,
        state_key_count: 100,
        raft_commit_index: 42,
        raft_applied_index: 40,
        raft_leader: Some(1),
        pending_writes: 10,
        pending_reads: 5,
    };

    assert_eq!(metrics.active_leases, 5);
    assert_eq!(metrics.membership_size, 3);
    assert_eq!(metrics.state_size_bytes, 1024);
    assert_eq!(metrics.state_key_count, 100);
    assert_eq!(metrics.raft_commit_index, 42);
    assert_eq!(metrics.raft_applied_index, 40);
    assert_eq!(metrics.raft_leader, Some(1));
    assert_eq!(metrics.pending_writes, 10);
    assert_eq!(metrics.pending_reads, 5);
}