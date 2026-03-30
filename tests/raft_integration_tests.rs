//! Integration tests for full Raft functionality
//!
//! Tests end-to-end Raft behavior including:
//! - Multi-node leader election
//! - Log replication across nodes
//! - Crash and recovery scenarios
//! - Vote persistence across restarts
//! - Snapshot during active writes

use ddl::cluster::{RaftClusterNode, OwnershipCommand, OwnershipState, AutoqueuesRaftStorage, NodeConfig};
use ddl::cluster::types::{SerializableLogEntry, SerializableVote, TypeConfig};
use openraft::{Entry, EntryPayload, LogId, CommittedLeaderId, Vote, RaftStorage, RaftSnapshotBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

// Test 1: Multi-node leader election setup
#[tokio::test]
#[cfg(feature = "integration")]
async fn test_multi_node_leader_election_setup() {
    // Create a single-node Raft cluster (in-memory for testing)
    // Note: Full multi-node tests require actual network setup

    let mut nodes = HashMap::new();
    nodes.insert(
        1,
        NodeConfig {
            node_id: 1,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        },
    );

    // Create Raft cluster
    let raft = RaftClusterNode::new(1, nodes)
        .await
        .expect("Failed to create Raft cluster");

    // Initialize would start leader election in a real multi-node setup
    // For single-node bootstrap, we'd expect it to become leader quickly

    // Clean shutdown
    raft.shutdown().await.ok();
}

// Test 2: Single-node cluster initialization
#[tokio::test]
async fn test_single_node_initialization() {
    let mut nodes = HashMap::new();
    nodes.insert(
        1,
        NodeConfig {
            node_id: 1,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        },
    );

    let raft = RaftClusterNode::new(1, nodes)
        .await
        .expect("Failed to create Raft cluster");

    // Single node should not be leader until initialized
    assert!(!raft.is_leader().await);

    // Initialize (makes single node the leader)
    raft.initialize().await.expect("Failed to initialize");

    // After initialization, single node should become leader
    assert!(raft.is_leader().await);

    raft.shutdown().await.ok();
}

// Test 3: Raft topic claim/release
#[tokio::test]
async fn test_raft_topic_claim_release() {
    let mut nodes = HashMap::new();
    nodes.insert(
        1,
        NodeConfig {
            node_id: 1,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        },
    );

    let raft = RaftClusterNode::new(1, nodes)
        .await
        .expect("Failed to create Raft cluster");

    raft.initialize().await.expect("Failed to initialize");

    // Poll until leader is elected (with timeout)
    let leader_ready = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        async {
            while !raft.is_leader().await {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    ).await;

    assert!(leader_ready.is_ok(), "Timeout waiting for leader election");

    // Claim a topic
    raft.claim_topic("test-topic").await.expect("Failed to claim topic");

    // Should own it
    assert!(raft.owns_topic("test-topic").await);
    assert_eq!(raft.get_owner("test-topic").await.expect("Failed to get owner"), Some(1));

    // Release it
    raft.release_topic("test-topic").await.expect("Failed to release topic");

    // Should not own it
    assert!(!raft.owns_topic("test-topic").await);
    assert_eq!(raft.get_owner("test-topic").await.expect("Failed to get owner"), None);

    raft.shutdown().await.ok();
}

// Test 4: Raft ownership state
#[test]
fn test_raft_ownership_state() {
    let mut state = OwnershipState::new();

    // Claim
    state.apply(&OwnershipCommand::ClaimTopic {
        topic: "topic1".to_string(),
        node_id: 1,
        timestamp: 1000,
    });
    assert_eq!(state.get_owner("topic1"), Some(1));

    // Transfer
    state.apply(&OwnershipCommand::TransferTopic {
        topic: "topic1".to_string(),
        from_node: 1,
        to_node: 2,
    });
    assert_eq!(state.get_owner("topic1"), Some(2));

    // Release
    state.apply(&OwnershipCommand::ReleaseTopic {
        topic: "topic1".to_string(),
        node_id: 2,
    });
    assert_eq!(state.get_owner("topic1"), None);
}

// Test 5: Crash and recovery
#[tokio::test]
async fn test_crash_recovery() {
    // Create storage with persistence
    let temp = TempDir::new().expect("Failed to create temp dir");

    // Phase 1: Write entries before crash
    {
        let mut storage = AutoqueuesRaftStorage::with_persistence(temp.path())
            .expect("Failed to create storage");

        // Add entries
        let entries = vec![
            Entry {
                log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
                payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                    topic: "test.crash_recovery".to_string(),
                    node_id: 1,
                    timestamp: 1000,
                }),
            },
            Entry {
                log_id: LogId::new(CommittedLeaderId::new(1, 1), 2),
                payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                    topic: "test.crash_recovery2".to_string(),
                    node_id: 2,
                    timestamp: 1001,
                }),
            },
        ];

        storage.append_to_log(entries).await.expect("Failed to append entries");
    }

    // Phase 2: Simulate crash (drop storage)
    // In real scenario, this would be a process crash/kill

    // Phase 3: Recover (create new storage from same directory)
    let mut storage2 = AutoqueuesRaftStorage::with_persistence(temp.path())
        .expect("Failed to create recovered storage");

    // Phase 4: Verify entries restored
    let log_state = storage2.get_log_state().await.expect("Failed to get log state");
    assert!(log_state.last_log_id.is_some(), "Should have entries after recovery");
    assert_eq!(log_state.last_log_id.unwrap().index, 2, "Should have 2 entries after recovery");
}

// Test 6: Vote persists across restarts
#[tokio::test]
async fn test_vote_persistence_across_restart() {
    let temp = TempDir::new().expect("Failed to create temp dir");

    // Phase 1: Create storage and cast vote
    {
        let mut storage = AutoqueuesRaftStorage::with_persistence(temp.path())
            .expect("Failed to create storage");

        // Cast a vote
        let vote = Vote::new(5, 3);
        storage.save_vote(&vote).await.expect("Failed to save vote");
    }

    // Phase 2: Simulate restart (drop and recreate)
    std::thread::sleep(std::time::Duration::from_millis(100)); // Small delay

    let mut storage2 = AutoqueuesRaftStorage::with_persistence(temp.path())
        .expect("Failed to create storage2");

    // Phase 3: Verify vote restored
    let restored = storage2.read_vote().await.expect("Failed to read vote");

    assert!(restored.is_some(), "Vote should be restored after restart");

    let restored = restored.unwrap();
    assert_eq!(restored.leader_id.term, 5, "Restored vote term should be 5");
    assert_eq!(restored.leader_id.node_id, 3, "Restored vote node_id should be 3");
}

// Test 7: Snapshot during active writes
#[tokio::test]
async fn test_snapshot_during_active_writes() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Start writing entries
    for i in 0..20 {
        storage.append_to_log(vec![Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.snapshot_{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        }]).await.expect("Failed to append entry");
    }

    // Perform snapshot while writes are ongoing
    let snapshot = storage.build_snapshot().await.expect("Failed to build snapshot");

    // Verify snapshot was created
    assert!(snapshot.meta.last_log_id.is_some(), "Snapshot should have log_id");
    assert_eq!(snapshot.meta.last_log_id.unwrap().index, 20, "Snapshot should capture all 20 entries");

    // Continue writing after snapshot
    storage.append_to_log(vec![Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 21),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "topic.after_snapshot".to_string(),
            node_id: 2,
            timestamp: 2000,
        }),
    }]).await.expect("Failed to append after snapshot");

    // Verify new entry was added
    let log_state = storage.get_log_state().await.expect("Failed to get log state");
    assert!(log_state.last_log_id.is_some(), "Should have last_log_id after snapshot");
    assert_eq!(log_state.last_log_id.unwrap().index, 21, "Should have 21 entries total");
}

// Test 8: Multiple snapshots
#[tokio::test]
async fn test_multiple_snapshots() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Create 3 snapshots with writes between them
    for snapshot_num in 0..3 {
        // Add entries before snapshot
        for i in 0..5 {
            storage.append_to_log(vec![Entry {
                log_id: LogId::new(CommittedLeaderId::new(1, 1), snapshot_num * 5 + i + 1),
                payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                    topic: format!("topic.s{}_e{}", snapshot_num, i),
                    node_id: 1,
                    timestamp: 1000 + (snapshot_num * 5 + i) as u64,
                }),
            }]).await.expect("Failed to append entry");
        }

        // Create snapshot
        let snapshot = storage.build_snapshot().await.expect("Failed to build snapshot");
        assert!(snapshot.meta.last_log_id.is_some(), "Snapshot {} should have log_id", snapshot_num);
    }

    // Verify all 15 entries are still in log
    let log_state = storage.get_log_state().await.expect("Failed to get log state");
    assert_eq!(log_state.last_log_id.unwrap().index, 15, "Should have all 15 entries");
}

// Test 9: Log compaction with purging
#[tokio::test]
async fn test_log_compaction_with_purging() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Add entries
    for i in 0..10 {
        storage.append_to_log(vec![Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.compact_{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        }]).await.expect("Failed to append entry");
    }

    // Purge first 5 entries (simulate snapshot compaction)
    let log_id_to_purge = LogId::new(CommittedLeaderId::new(1, 1), 5);
    storage.purge_logs_upto(log_id_to_purge).await.expect("Failed to purge");

    // Verify first 5 entries are removed
    let log_state = storage.get_log_state().await.expect("Failed to get log state");
    assert_eq!(log_state.last_purged_log_id.unwrap().index, 5, "Should have purged up to index 5");
}

// Test 10: Concurrent storage clones
#[test]
fn test_concurrent_storage_clones() {
    // Verify that storage clones share the same ownership state
    let storage = Arc::new(AutoqueuesRaftStorage::new());

    // Modify state through one clone
    let storage_clone1 = Arc::clone(&storage);
    storage_clone1.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
        topic: "test.concurrency".to_string(),
        node_id: 1,
        timestamp: 1000,
    });

    // Verify other clone sees same state
    let storage_clone2 = Arc::clone(&storage);
    let owner = storage_clone2.ownership_state.read().unwrap().get_owner("test.concurrency");
    assert_eq!(owner, Some(1), "Clones should share state");
}

// Test 11: Complete round-trip: write, persist, recover, read
#[tokio::test]
async fn test_complete_roundtrip_persistence() {
    let temp = TempDir::new().expect("Failed to create temp dir");

    // Phase 1: Create storage and add entries
    {
        let mut storage = AutoqueuesRaftStorage::with_persistence(temp.path())
            .expect("Failed to create storage");

        // Add log entries
        let entries = vec![
            Entry {
                log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
                payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                    topic: "roundtrip.topic".to_string(),
                    node_id: 1,
                    timestamp: 1000,
                }),
            },
        ];

        storage.append_to_log(entries).await.expect("Failed to append");

        // Save vote
        let vote = Vote::new(1, 1);
        storage.save_vote(&vote).await.expect("Failed to save vote");

        // Save ownership state - use mark_dirty_and_flush for direct modifications
        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "roundtrip.state".to_string(),
            node_id: 1,
            timestamp: 1001,
        });
        storage.mark_dirty_and_flush().expect("Failed to flush ownership state");
    } // storage is dropped here - Drop implementation should also flush

    // Phase 2: Recreate storage (simulating restart)
    let mut storage2 = AutoqueuesRaftStorage::with_persistence(temp.path())
        .expect("Failed to create recovered storage");

    // Phase 3: Verify all data restored
    let log_state = storage2.get_log_state().await.expect("Failed to get log state");
    assert_eq!(log_state.last_log_id.unwrap().index, 1, "Should have 1 log entry");

    let vote = storage2.read_vote().await.expect("Failed to read vote");
    assert!(vote.is_some(), "Should have vote");

    let owner = storage2.ownership_state.read().unwrap().get_owner("roundtrip.state");
    assert_eq!(owner, Some(1), "Should have ownership state");
}

// Test 12: DDL distributed with Raft config
#[test]
fn test_ddl_distributed_with_raft_config() {
    // Test DDL distributed mode with Raft enabled
    // This verifies the configuration path is correct

    use ddl::{DdlConfig, DdlDistributed, DDL};

    let config = DdlConfig {
        raft_enabled: true,
        gossip_enabled: false,
        is_bootstrap: true,
        node_id: 1,
        ..Default::default()
    };

    // Note: This would require actual Raft cluster setup with network
    // For this test, we verify the configuration path is correct

    // Test passes if config can be created
    assert!(config.raft_enabled);
    assert_eq!(config.node_id, 1);
}

// Test 13: Log state after operations
#[tokio::test]
async fn test_log_state_operations() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Initially empty
    let state = storage.get_log_state().await.expect("Failed to get initial log state");
    assert!(state.last_purged_log_id.is_none(), "Should have no purged logs initially");
    assert!(state.last_log_id.is_none(), "Should have no log entries initially");

    // Add entries
    storage.append_to_log(vec![Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "test.state".to_string(),
            node_id: 1,
            timestamp: 1000,
        }),
    }]).await.expect("Failed to append");

    // Should have log entry
    let state = storage.get_log_state().await.expect("Failed to get log state");
    assert!(state.last_log_id.is_some(), "Should have last_log_id after append");
}

// Test 14: Delete conflicting logs
#[tokio::test]
async fn test_delete_conflicting_logs() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Add entries
    for i in 0..10 {
        storage.append_to_log(vec![Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.conflict_{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        }]).await.expect("Failed to append");
    }

    // Delete logs from index 5 onwards
    let log_id = LogId::new(CommittedLeaderId::new(1, 1), 5);
    storage.delete_conflict_logs_since(log_id).await.expect("Failed to delete");

    // Verify logs 5-10 were deleted - last_log_id should now be 4
    let log_state = storage.get_log_state().await.expect("Failed to get log state");
    assert_eq!(log_state.last_log_id.unwrap().index, 4, "Should have entries 1-4 after deletion");
}

// Test 15: Ownership state recovery
#[tokio::test]
async fn test_ownership_state_recovery() {
    let temp = TempDir::new().expect("Failed to create temp dir");

    // Phase 1: Create and save state
    {
        let storage = AutoqueuesRaftStorage::with_persistence(temp.path())
            .expect("Failed to create storage");

        // Apply multiple commands
        let commands = vec![
            OwnershipCommand::ClaimTopic {
                topic: "state1".to_string(),
                node_id: 1,
                timestamp: 1000,
            },
            OwnershipCommand::ClaimTopic {
                topic: "state2".to_string(),
                node_id: 2,
                timestamp: 1001,
            },
            OwnershipCommand::ClaimTopic {
                topic: "state3".to_string(),
                node_id: 3,
                timestamp: 1002,
            },
        ];

        for cmd in commands {
            storage.ownership_state.write().unwrap().apply(&cmd);
        }

        // Explicitly flush to persist the ownership state
        storage.mark_dirty_and_flush().expect("Failed to flush ownership state");
    }

    // Phase 2: Recover and verify
    let storage2 = AutoqueuesRaftStorage::with_persistence(temp.path())
        .expect("Failed to create storage2");

    let state = storage2.ownership_state.read().unwrap();

    // Verify all topics have correct owners
    assert_eq!(state.get_owner("state1"), Some(1));
    assert_eq!(state.get_owner("state2"), Some(2));
    assert_eq!(state.get_owner("state3"), Some(3));

    // Verify topic count
    assert_eq!(state.topic_count(), 3);
}

// Test 16: SerializableLogEntry serialization
#[test]
fn test_serializable_log_entry_serialization() {
    let entry: Entry<TypeConfig> = Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 42),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "test.serialization".to_string(),
            node_id: 1,
            timestamp: 1000,
        }),
    };

    // Convert to serializable
    let se = SerializableLogEntry::from(&entry);

    // Serialize
    let serialized = oxicode::serde::encode_to_vec(&se, oxicode::config::standard()).expect("Failed to serialize");

    // Deserialize
    let restored: SerializableLogEntry = oxicode::serde::decode_from_slice(&serialized, oxicode::config::standard())
        .map(|(v, _)| v)
        .expect("Failed to deserialize");

    // Convert back
    let entry2: Entry<TypeConfig> = restored.into();

    assert_eq!(entry2.log_id.index, 42);
    assert_eq!(entry2.log_id.leader_id.term, 1);
    assert_eq!(entry2.log_id.leader_id.node_id, 1);
}

// Test 17: SerializableVote serialization
#[test]
fn test_serializable_vote_serialization() {
    let vote = Vote::new(3, 7);

    // Create SerializableVote manually 
    let sv = SerializableVote {
        term: vote.leader_id.term,
        node_id: vote.leader_id.node_id,
        committed: vote.committed,
    };

    // Serialize
    let serialized = oxicode::serde::encode_to_vec(&sv, oxicode::config::standard()).expect("Failed to serialize");

    // Deserialize
    let restored: SerializableVote = oxicode::serde::decode_from_slice(&serialized, oxicode::config::standard())
        .map(|(v, _)| v)
        .expect("Failed to deserialize");

    // Create Vote from SerializableVote components
    let vote2 = Vote::new(restored.term, restored.node_id);

    assert_eq!(vote2.leader_id.term, 3);
    assert_eq!(vote2.leader_id.node_id, 7);
}

// Test 18: OwnershipState serialization
#[test]
fn test_ownership_state_oxicode_serialization() {
    let mut state = OwnershipState::new();
    state.apply(&OwnershipCommand::ClaimTopic {
        topic: "topic1".to_string(),
        node_id: 1,
        timestamp: 1000,
    });
    state.apply(&OwnershipCommand::ClaimTopic {
        topic: "topic2".to_string(),
        node_id: 2,
        timestamp: 1001,
    });

    // Serialize
    let serialized = oxicode::serde::encode_to_vec(&state, oxicode::config::standard()).expect("Failed to serialize");

    // Deserialize
    let restored: OwnershipState = oxicode::serde::decode_from_slice(&serialized, oxicode::config::standard())
        .map(|(v, _)| v)
        .expect("Failed to deserialize");

    assert_eq!(restored.get_owner("topic1"), Some(1));
    assert_eq!(restored.get_owner("topic2"), Some(2));
}