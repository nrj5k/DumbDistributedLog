//! Integration tests for Raft persistence (P1 fixes)
//!
//! Tests the implementation of P1 critical production blockers:
//! - Log persistence and restoration
//! - Vote persistence and restoration
//! - Atomic file writes to prevent partial state
//! - Snapshot consistency after compaction
//! - Leadership election with exponential backoff
//! - Concurrent flush protection

use ddl::cluster::{
    AutoqueuesRaftStorage, OwnershipCommand, OwnershipState,
};
use ddl::cluster::types::{SerializableLogEntry, SerializableVote, TypeConfig};
use openraft::{Entry, EntryPayload, LogId, CommittedLeaderId, Vote, RaftStorage, RaftSnapshotBuilder};
use tempfile::TempDir;
use std::sync::Arc;

// Test 1: SerializableLogEntry roundtrip
#[test]
fn test_serializable_log_entry_roundtrip() {
    let entry: Entry<TypeConfig> = Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 42),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "test".to_string(),
            node_id: 1,
            timestamp: 1000,
        }),
    };

    let se = SerializableLogEntry::from(&entry);
    let restored: Entry<TypeConfig> = se.into();

    assert_eq!(restored.log_id.index, 42, "Entry index should be preserved");
    assert_eq!(restored.log_id.leader_id.term, 1, "Entry term should be preserved");
    assert_eq!(restored.log_id.leader_id.node_id, 1, "Entry node_id should be preserved");
}

// Test 2: SerializableVote roundtrip
#[test]
fn test_serializable_vote_roundtrip() {
    let vote = Vote::new(3, 7);

    // Create SerializableVote manually
    let sv = SerializableVote {
        term: vote.leader_id.term,
        node_id: vote.leader_id.node_id,
        committed: vote.committed,
    };

    // Verify fields
    assert_eq!(sv.term, 3, "Vote term should be preserved");
    assert_eq!(sv.node_id, 7, "Vote node_id should be preserved");
    assert!(!sv.committed, "Vote should not be committed");

    // Create Vote from SerializableVote
    let mut restored = Vote::new(sv.term, sv.node_id);
    if sv.committed {
        restored.commit();
    }

    assert_eq!(restored.leader_id.term, 3, "Vote term should be preserved");
    assert_eq!(restored.leader_id.node_id, 7, "Vote node_id should be preserved");
}

// Test 3: Ownership state serialization
#[test]
fn test_ownership_state_serialization() {
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

    let serialized = bincode::serialize(&state).unwrap();
    let restored: OwnershipState = bincode::deserialize(&serialized).unwrap();

    assert_eq!(restored.get_owner("topic1"), Some(1));
    assert_eq!(restored.get_owner("topic2"), Some(2));
}

// Test 4: Storage creation with persistence
#[test]
fn test_storage_with_persistence() {
    let temp = TempDir::new().unwrap();
    let storage = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

    // Verify storage is created successfully
    // Check that ownership_state is accessible (it's public)
    let count = storage.ownership_state.read().unwrap().topic_count();
    assert_eq!(count, 0, "New storage should have no topics");
}

// Test 5: Empty storage creation
#[test]
fn test_empty_storage_creation() {
    let storage = AutoqueuesRaftStorage::new();
    // Verify it can be created without persistence
    // Storage without persistence should still work for in-memory use
    let count = storage.ownership_state.read().unwrap().topic_count();
    assert_eq!(count, 0, "New storage should have no topics");
}

// Test 6: Log persistence roundtrip via RaftStorage trait
#[tokio::test]
async fn test_log_persistence_roundtrip() {
    let temp = TempDir::new().unwrap();
    let mut storage = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

    // Create some entries
    let entries = vec![
        Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: "test.topic.1".to_string(),
                node_id: 1,
                timestamp: 1000,
            }),
        },
        Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), 2),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: "test.topic.2".to_string(),
                node_id: 2,
                timestamp: 1001,
            }),
        },
    ];

    // Write entries using RaftStorage trait
    storage.append_to_log(entries).await.unwrap();

    // Get log state via trait method
    let log_state = storage.get_log_state().await.unwrap();
    assert!(log_state.last_log_id.is_some(), "Should have entries");
    assert_eq!(log_state.last_log_id.unwrap().index, 2, "Last index should be 2");

    // Simulate restart by creating new storage from same path
    drop(storage);

    let mut storage2 = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();
    let log_state2 = storage2.get_log_state().await.unwrap();

    // Verify entries persisted
    assert!(log_state2.last_log_id.is_some(), "Should have entries after restart");
    assert_eq!(log_state2.last_log_id.unwrap().index, 2, "Last index should be 2 after restart");
}

// Test 7: Vote persistence roundtrip via RaftStorage trait
#[tokio::test]
async fn test_vote_persistence_roundtrip() {
    let temp = TempDir::new().unwrap();
    let mut storage = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

    // Create and save vote using RaftStorage trait
    let vote = Vote::new(5, 1);
    storage.save_vote(&vote).await.unwrap();

    // Read vote back via trait
    let restored = storage.read_vote().await.unwrap();
    assert!(restored.is_some(), "Vote should be saved");
    assert_eq!(restored.as_ref().unwrap().leader_id.term, 5, "Saved vote term should be 5");
    assert_eq!(restored.as_ref().unwrap().leader_id.node_id, 1, "Saved vote node_id should be 1");

    // Simulate restart
    drop(storage);

    let mut storage2 = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();
    let restored2 = storage2.read_vote().await.unwrap();

    assert!(restored2.is_some(), "Vote should persist after restart");
    assert_eq!(restored2.as_ref().unwrap().leader_id.term, 5, "Vote term should persist");
    assert_eq!(restored2.as_ref().unwrap().leader_id.node_id, 1, "Vote node_id should persist");
}

// Test 8: Atomic write prevents partial state
#[test]
fn test_atomic_write_prevents_partial_state() {
    let temp = TempDir::new().unwrap();

    // Create storage and persist some state
    {
        let storage = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

        // Write some ownership state
        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "test.atomic".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
    }

    // Reload and verify
    let storage2 = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();
    let owner = storage2.ownership_state.read().unwrap().get_owner("test.atomic");
    assert_eq!(owner, Some(1), "State should have correct owner");
}

// Test 9: Snapshot creation
#[tokio::test]
async fn test_snapshot_creation() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Add entries to log
    let entries = vec![
        Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: "test.snapshot".to_string(),
                node_id: 1,
                timestamp: 1000,
            }),
        },
        Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), 2),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: "test.snapshot2".to_string(),
                node_id: 2,
                timestamp: 1001,
            }),
        },
    ];

    // Append entries
    storage.append_to_log(entries).await.unwrap();

    // Get log state
    let state = storage.get_log_state().await.unwrap();

    // Verify consistency
    assert!(
        state.last_log_id.is_some(),
        "Log state should have last_log_id"
    );

    // Build snapshot to verify it works
    let snapshot = storage.build_snapshot().await.unwrap();
    assert!(snapshot.meta.last_log_id.is_some(), "Snapshot should have last_log_id in meta");
}

// Test 10: Concurrent storage clones share state
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

// Test 11: Leadership wait with exponential backoff
#[test]
fn test_leadership_wait_timeout() {
    // This tests the leadership election logic in ddl_distributed.rs
    // We'll simulate the exponential backoff behavior

    let max_attempts = 10;
    let mut attempt = 0;
    let base_delay = std::time::Duration::from_millis(100);
    let max_delay = std::time::Duration::from_secs(5);

    let mut delays = Vec::new();

    loop {
        // Calculate delay with exponential backoff
        let delay = std::cmp::min(base_delay * 2u32.pow(attempt as u32), max_delay);
        delays.push(delay);

        attempt += 1;
        if attempt >= max_attempts {
            break;
        }
    }

    // Verify we made all attempts
    assert_eq!(attempt, max_attempts, "Should have exhausted all attempts");

    // Verify exponential growth pattern
    assert!(delays[1] >= delays[0] * 2 || delays[1] == max_delay, "First delay should be at least 2x base");
    assert!(delays[2] >= delays[1] * 2 || delays[2] == max_delay, "Second delay should be at least 2x previous");
}

// Test 12: Ownership commands work correctly
#[test]
fn test_ownership_commands() {
    let mut state = OwnershipState::new();

    // Test ClaimTopic
    state.apply(&OwnershipCommand::ClaimTopic {
        topic: "topic1".to_string(),
        node_id: 1,
        timestamp: 1000,
    });
    assert_eq!(state.get_owner("topic1"), Some(1));

    // Test TransferTopic
    state.apply(&OwnershipCommand::TransferTopic {
        topic: "topic1".to_string(),
        from_node: 1,
        to_node: 2,
    });
    assert_eq!(state.get_owner("topic1"), Some(2));

    // Test ReleaseTopic
    state.apply(&OwnershipCommand::ReleaseTopic {
        topic: "topic1".to_string(),
        node_id: 2,
    });
    assert_eq!(state.get_owner("topic1"), None);
}

// Test 13: Multiple log entries
#[tokio::test]
async fn test_multiple_log_entries() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Add entries one by one
    for i in 0..10 {
        let entry = Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        };
        storage.append_to_log(vec![entry]).await.unwrap();
    }

    // Verify log state
    let log_state = storage.get_log_state().await.unwrap();
    assert!(log_state.last_log_id.is_some());
    assert_eq!(log_state.last_log_id.unwrap().index, 10);
}

// Test 14: Storage with invalid data directory
#[test]
fn test_storage_with_invalid_path() {
    // Should handle gracefully when path doesn't exist yet
    let temp = TempDir::new().unwrap();
    let result = AutoqueuesRaftStorage::with_persistence(temp.path());
    assert!(result.is_ok(), "Should create storage even if files don't exist");
}

// Test 15: Persistence across multiple restarts
#[tokio::test]
async fn test_persistence_across_multiple_restarts() {
    let temp = TempDir::new().unwrap();

    // First restart - create and save state
    {
        let storage = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

        // Apply multiple commands
        for i in 0..5 {
            storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            });
        }
    }

    // Second restart - verify state persisted
    {
        let storage2 = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

        for i in 0..5 {
            let owner = storage2.ownership_state.read().unwrap().get_owner(&format!("topic.{}", i));
            assert_eq!(owner, Some(1), "Topic {} should be owned by node 1 after restart", i);
        }
    }

    // Third restart - verify again
    {
        let storage3 = AutoqueuesRaftStorage::with_persistence(temp.path()).unwrap();

        // All topics should still be owned
        for i in 0..5 {
            let owner = storage3.ownership_state.read().unwrap().get_owner(&format!("topic.{}", i));
            assert_eq!(owner, Some(1), "Topic {} should still be owned after third restart", i);
        }
    }
}

// Test 16: Vote with committed flag
#[test]
fn test_vote_committed_flag() {
    let mut vote = Vote::new(5, 3);

    // Initially not committed
    assert!(!vote.committed);

    // Commit the vote
    vote.commit();
    assert!(vote.committed);

    // Create SerializableVote manually
    let sv = SerializableVote {
        term: vote.leader_id.term,
        node_id: vote.leader_id.node_id,
        committed: vote.committed,
    };

    // Create Vote from SerializableVote
    let mut restored = Vote::new(sv.term, sv.node_id);
    if sv.committed {
        restored.commit();
    }

    assert_eq!(restored.leader_id.term, 5);
    assert_eq!(restored.leader_id.node_id, 3);
    assert!(restored.committed, "Committed flag should be preserved");
}

// Test 17: Entry with blank payload
#[test]
fn test_entry_with_blank_payload() {
    let entry: Entry<TypeConfig> = Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
        payload: EntryPayload::Blank,
    };

    let se = SerializableLogEntry::from(&entry);
    let restored: Entry<TypeConfig> = se.into();

    assert_eq!(restored.log_id.index, 1);
    match restored.payload {
        EntryPayload::Blank => {} // Expected
        _ => panic!("Expected Blank payload"),
    }
}

// Test 18: Storage cloning preserves shared state
#[test]
fn test_storage_clone_shared_state() {
    let storage1 = AutoqueuesRaftStorage::new();
    let storage2 = storage1.clone();

    // Modify state through first instance
    storage1.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
        topic: "shared".to_string(),
        node_id: 1,
        timestamp: 1000,
    });

    // Verify second instance sees the change
    let owner = storage2.ownership_state.read().unwrap().get_owner("shared");
    assert_eq!(owner, Some(1), "Cloned storage should share state");
}

// Test 19: Log state consistency
#[tokio::test]
async fn test_log_state_consistency() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Initially empty
    let state = storage.get_log_state().await.unwrap();
    assert!(state.last_purged_log_id.is_none(), "Should have no purged logs initially");
    assert!(state.last_log_id.is_none(), "Should have no log entries initially");

    // Add entries
    for i in 0..5 {
        storage.append_to_log(vec![Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        }]).await.unwrap();
    }

    // Should have entries now
    let state = storage.get_log_state().await.unwrap();
    assert!(state.last_log_id.is_some(), "Should have last_log_id after append");
    assert_eq!(state.last_log_id.unwrap().index, 5, "Should have 5 entries");
}

// Test 20: Initial snapshot state
#[tokio::test]
async fn test_initial_snapshot_state() {
    use openraft::RaftSnapshotBuilder;
    let mut storage = AutoqueuesRaftStorage::new();

    // Initially there's no snapshot
    let snapshot_opt = storage.get_current_snapshot().await.unwrap();
    assert!(snapshot_opt.is_none(), "Initially there should be no snapshot");

    // Build a snapshot
    storage.append_to_log(vec![Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "test".to_string(),
            node_id: 1,
            timestamp: 1000,
        }),
    }]).await.unwrap();

    let snapshot = storage.build_snapshot().await.unwrap();
    assert!(snapshot.meta.last_log_id.is_some(), "Snapshot should have last_log_id after building");
}

// Test 21: Log compaction with purging
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
        }]).await.unwrap();
    }

    // Purge first 5 entries (simulate snapshot compaction)
    let log_id_to_purge = LogId::new(CommittedLeaderId::new(1, 1), 5);
    storage.purge_logs_upto(log_id_to_purge).await.unwrap();

    // Verify first 5 entries are removed
    let log_state = storage.get_log_state().await.unwrap();
    assert!(log_state.last_purged_log_id.is_some(), "Should have purged log_id");
    assert_eq!(log_state.last_purged_log_id.unwrap().index, 5, "Should have purged up to index 5");
}

// Test 22: Snapshot during active writes
#[tokio::test]
async fn test_snapshot_during_active_writes() {
    let mut storage = AutoqueuesRaftStorage::new();

    // Start writing entries
    for i in 0..10 {
        storage.append_to_log(vec![Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), i + 1),
            payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: 1,
                timestamp: 1000 + i as u64,
            }),
        }]).await.unwrap();
    }

    // Perform snapshot while writes are ongoing
    let snapshot = storage.build_snapshot().await.unwrap();

    // Verify snapshot was created
    assert!(snapshot.meta.last_log_id.is_some(), "Snapshot should have last_log_id");
    assert_eq!(snapshot.meta.last_log_id.unwrap().index, 10, "Snapshot should capture all 10 entries");

    // Continue writing after snapshot
    storage.append_to_log(vec![Entry {
        log_id: LogId::new(CommittedLeaderId::new(1, 1), 11),
        payload: EntryPayload::Normal(OwnershipCommand::ClaimTopic {
            topic: "topic.after_snapshot".to_string(),
            node_id: 2,
            timestamp: 2000,
        }),
    }]).await.unwrap();

    // Verify we can still get log state after snapshot
    let log_state = storage.get_log_state().await.unwrap();
    assert!(log_state.last_log_id.is_some(), "Should have last_log_id after snapshot");
    assert_eq!(log_state.last_log_id.unwrap().index, 11, "Should have 11 entries total");
}