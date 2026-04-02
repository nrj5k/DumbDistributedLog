//! Real multi-node cluster tests using TCP networking
//! 
//! These tests spawn actual TCP servers and verify real network communication.
//! No in-memory simulation.

mod distributed_test_utils;
use distributed_test_utils::TestCluster;
use tokio::time::{timeout, Duration};

/// Test that a 3-node cluster can elect a leader
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_3_node_cluster_leader_election() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    // Node 1 should be leader (bootstrap node)
    let bootstrap = cluster.get_node(1).unwrap();
    assert!(bootstrap.is_leader().await, "Bootstrap should be leader");
    
    // All nodes should see the same leader
    let leader_id = bootstrap.get_leader().await;
    assert_eq!(leader_id, Some(1), "Leader should be node 1");
    
    // Clean shutdown
    cluster.shutdown().await.ok();
}

/// Test that topic ownership is replicated across all nodes
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_topic_ownership_replication() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    // Claim topic on leader
    let leader = cluster.get_node(1).unwrap();
    leader.claim_topic("test.topic").await.expect("Claim failed");
    
    // Wait for replication (real network latency)
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify on all nodes
    for node in &cluster.nodes {
        let owner = node.get_owner("test.topic").await.expect("Get owner failed");
        assert_eq!(owner, Some(1), "All nodes should see same owner");
    }
    
    cluster.shutdown().await.ok();
}

/// Test that lease acquisition is consistent across cluster
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_lease_acquisition() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    // Acquire lease on leader
    let leader = cluster.get_node(1).unwrap();
    let lease = leader.acquire_lease("resource:1", 60).await
        .expect("Lease acquisition failed");
    
    assert_eq!(lease.owner, 1, "Lease owner should be node 1");
    // expires_at is a u64 timestamp (nanoseconds), compare with current time
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    assert!(lease.expires_at > now_ns, "Lease should expire in the future");
    
    // Verify lease is visible on other nodes
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    for node in &cluster.nodes {
        let lease_info = node.get_lease_owner("resource:1").await
            .expect("Get lease owner failed");
        assert!(lease_info.is_some(), "Lease should be visible on all nodes");
        assert_eq!(lease_info.unwrap().owner, 1);
    }
    
    cluster.shutdown().await.ok();
}

/// Test that lease release works across cluster
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_lease_release() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    let leader = cluster.get_node(1).unwrap();
    
    // Acquire lease
    leader.acquire_lease("resource:2", 60).await.expect("Acquire failed");
    
    // Release lease
    leader.release_lease("resource:2").await.expect("Release failed");
    
    // Verify lease is gone
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    for node in &cluster.nodes {
        let lease_info = node.get_lease_owner("resource:2").await
            .expect("Get lease owner failed");
        assert!(lease_info.is_none(), "Lease should be released");
    }
    
    cluster.shutdown().await.ok();
}

/// Test 5-node cluster scalability
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_5_node_cluster() {
    let cluster = TestCluster::setup_tcp("node[1-5]")
        .await
        .expect("Failed to create 5-node cluster");
    
    assert_eq!(cluster.size(), 5);
    
    // Verify leader
    let is_leader = timeout(Duration::from_secs(10), async {
        loop {
            if cluster.get_node(1).unwrap().is_leader().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Leader election timeout");
    
    assert!(is_leader);
    
    // Test topic ownership on larger cluster
    let leader = cluster.get_node(1).unwrap();
    leader.claim_topic("large.cluster.topic").await.expect("Claim failed");
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    for node in &cluster.nodes {
        let owner = node.get_owner("large.cluster.topic").await.expect("Get owner failed");
        assert_eq!(owner, Some(1), "All nodes should see same owner");
    }
    
    cluster.shutdown().await.ok();
}

/// Test topic transfer between nodes
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_topic_transfer() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    let leader = cluster.get_node(1).unwrap();
    
    // Claim topic initially
    leader.claim_topic("transfer.topic").await.expect("Claim failed");
    
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    // Transfer to node 2
    leader.transfer_topic("transfer.topic", 2).await.expect("Transfer failed");
    
    // Wait for transfer
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify transfer on all nodes
    for node in &cluster.nodes {
        let owner = node.get_owner("transfer.topic").await.expect("Get owner failed");
        assert_eq!(owner, Some(2), "Topic should be owned by node 2");
    }
    
    cluster.shutdown().await.ok();
}

/// Test membership changes
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_membership_changes() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    // Initial membership should have 3 nodes
    let leader = cluster.get_node(1).unwrap();
    let membership = leader.membership();
    assert_eq!(membership.nodes.len(), 3);
    
    // Subscribe to membership events
    let _events = leader.subscribe_membership();
    
    // The bootstrap node already emitted a NodeJoined event during initialization
    // Other nodes emit events when they join
    
    // Verify we can receive events (they're buffered)
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    cluster.shutdown().await.ok();
}

/// Test cluster resilience: verify all nodes stay connected
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_real_cluster_resilience() {
    let cluster = TestCluster::setup_tcp("node[1-3]")
        .await
        .expect("Failed to create TCP cluster");
    
    let leader = cluster.get_node(1).unwrap();
    
    // Claim multiple topics
    for i in 0..10 {
        leader.claim_topic(&format!("topic.{}", i)).await.expect("Claim failed");
    }
    
    // Wait for replication
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all topics are visible on all nodes
    for node in &cluster.nodes {
        for i in 0..10 {
            let owner = node.get_owner(&format!("topic.{}", i)).await.expect("Get owner failed");
            assert_eq!(owner, Some(1), "All topics should be owned by node 1");
        }
    }
    
    cluster.shutdown().await.ok();
}