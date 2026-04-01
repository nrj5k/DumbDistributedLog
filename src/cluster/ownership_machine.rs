//! Ownership state machine for Raft
//!
//! Provides strongly consistent topic ownership via Raft consensus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::lock_utils::Validate;

/// Unique node identifier
pub type NodeId = u64;

/// Lease entry stored in the state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseEntry {
    pub id: u64,
    pub key: String,
    pub owner: NodeId,
    pub acquired_at: u64, // nanos since epoch
    pub ttl_secs: u64,
    pub expires_at: u64, // acquired_at + ttl_secs * 1_000_000_000
}

/// Public lease info (returned to callers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseInfo {
    pub id: u64,
    pub key: String,
    pub owner: NodeId,
    pub acquired_at: u64,
    pub expires_at: u64,
    pub ttl_secs: u64,
}

/// Result of lease operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaseError {
    /// Lease for key is held by another node
    LeaseHeld { key: String, owner: NodeId },
    /// Lease has expired
    LeaseExpired(u64),
    /// Lease not found
    LeaseNotFound(u64),
}

impl From<LeaseEntry> for LeaseInfo {
    fn from(entry: LeaseEntry) -> Self {
        Self {
            id: entry.id,
            key: entry.key,
            owner: entry.owner,
            acquired_at: entry.acquired_at,
            expires_at: entry.expires_at,
            ttl_secs: entry.ttl_secs,
        }
    }
}

impl From<&LeaseEntry> for LeaseInfo {
    fn from(entry: &LeaseEntry) -> Self {
        Self {
            id: entry.id,
            key: entry.key.clone(),
            owner: entry.owner,
            acquired_at: entry.acquired_at,
            expires_at: entry.expires_at,
            ttl_secs: entry.ttl_secs,
        }
    }
}

/// Topic ownership command (applied via Raft)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OwnershipCommand {
    /// Claim ownership of a topic
    ClaimTopic {
        topic: String,
        node_id: NodeId,
        timestamp: u64,
    },

    /// Release ownership of a topic
    ReleaseTopic { topic: String, node_id: NodeId },

    /// Transfer ownership to another node (graceful handoff)
    TransferTopic {
        topic: String,
        from_node: NodeId,
        to_node: NodeId,
    },

    // ========================================================================
    // Lease Commands (TTL-based ownership for SCORE integration)
    // ========================================================================
    /// Acquire a lease with TTL - auto-expires if not renewed
    AcquireLease {
        key: String,
        owner: NodeId,
        lease_id: u64, // Monotonically increasing
        ttl_secs: u64,
        timestamp: u64,
    },

    /// Renew an existing lease
    RenewLease { lease_id: u64, timestamp: u64 },

    /// Release a lease gracefully
    ReleaseLease { key: String },

    /// Expire all stale leases (called periodically)
    ExpireLeases { now: u64 },
}

/// Query for ownership state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OwnershipQuery {
    /// Get owner of a specific topic
    GetOwner { topic: String },

    /// List all topics owned by a node
    ListTopics { node_id: NodeId },

    /// List all ownership mappings
    ListAll,

    // ========================================================================
    // Lease Queries (TTL-based ownership for SCORE integration)
    // ========================================================================
    /// Get current lease holder for a key
    GetLeaseOwner { key: String },

    /// List all leases owned by a node
    ListLeases { owner: NodeId },

    /// Get lease by ID
    GetLease { lease_id: u64 },
}

/// Query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OwnershipResponse {
    Owner {
        topic: String,
        owner: Option<NodeId>,
    },
    Topics {
        topics: Vec<String>,
    },
    All {
        ownership: HashMap<String, NodeId>,
    },

    // ========================================================================
    // Lease Responses (TTL-based ownership for SCORE integration)
    // ========================================================================
    /// Lease owner for a key
    LeaseOwner {
        key: String,
        info: Option<LeaseInfo>,
    },
    /// List of leases for a node
    Leases {
        leases: Vec<LeaseInfo>,
    },
    /// Lease entry result
    LeaseResult {
        lease: Option<LeaseEntry>,
        error: Option<LeaseError>,
    },
    /// Count of expired leases
    LeaseCount {
        count: usize,
    },
}

/// Ownership state (the state machine)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OwnershipState {
    /// topic -> owner_node_id
    ownership: HashMap<String, NodeId>,
    /// node_id -> topics (reverse index for fast lookup)
    node_topics: HashMap<NodeId, Vec<String>>,

    // Lease management for SCORE integration
    /// lease_id -> lease entry
    leases: HashMap<u64, LeaseEntry>,
    /// key -> lease_id (one lease per key invariant)
    key_leases: HashMap<String, u64>,
    /// Monotonic counter for lease IDs
    next_lease_id: u64,
}

impl OwnershipState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a command to the state machine
    pub fn apply(&mut self, cmd: &OwnershipCommand) {
        match cmd {
            OwnershipCommand::ClaimTopic { topic, node_id, .. } => {
                // Remove from previous owner if any
                if let Some(prev_owner) = self.ownership.get(topic) {
                    if let Some(topics) = self.node_topics.get_mut(prev_owner) {
                        topics.retain(|t| t != topic);
                    }
                }

                // Set new owner
                self.ownership.insert(topic.clone(), *node_id);
                self.node_topics
                    .entry(*node_id)
                    .or_insert_with(Vec::new)
                    .push(topic.clone());
            }

            OwnershipCommand::ReleaseTopic { topic, node_id } => {
                // Only release if we're the owner
                if self.ownership.get(topic) == Some(node_id) {
                    self.ownership.remove(topic);
                    if let Some(topics) = self.node_topics.get_mut(node_id) {
                        topics.retain(|t| t != topic);
                    }
                }
            }

            OwnershipCommand::TransferTopic {
                topic,
                from_node,
                to_node,
            } => {
                // Only transfer if from_node is the current owner
                if self.ownership.get(topic) == Some(from_node) {
                    self.ownership.insert(topic.clone(), *to_node);

                    // Remove from from_node
                    if let Some(topics) = self.node_topics.get_mut(from_node) {
                        topics.retain(|t| t != topic);
                    }

                    // Add to to_node
                    self.node_topics
                        .entry(*to_node)
                        .or_insert_with(Vec::new)
                        .push(topic.clone());
                }
            }

            // ====================================================================
            // Lease Commands
            // ====================================================================
            OwnershipCommand::AcquireLease {
                key,
                owner,
                lease_id,
                ttl_secs,
                timestamp,
            } => {
                // Check if key already has a lease
                if let Some(existing_id) = self.key_leases.get(key) {
                    if let Some(existing) = self.leases.get(existing_id) {
                        // If lease is still valid, don't acquire
                        if existing.expires_at > *timestamp {
                            return; // Lease held, command rejected
                        }
                        // Lease expired, will be cleaned up by AcquireLease
                    }
                }

                // Create new lease
                let expires_at = timestamp.saturating_add(ttl_secs.saturating_mul(1_000_000_000));
                let lease = LeaseEntry {
                    id: *lease_id,
                    key: key.clone(),
                    owner: *owner,
                    acquired_at: *timestamp,
                    ttl_secs: *ttl_secs,
                    expires_at,
                };

                self.leases.insert(*lease_id, lease);
                self.key_leases.insert(key.clone(), *lease_id);

                // Update next_lease_id if this ID is higher
                if *lease_id >= self.next_lease_id {
                    self.next_lease_id = lease_id + 1;
                }
            }

            OwnershipCommand::RenewLease {
                lease_id,
                timestamp,
            } => {
                if let Some(lease) = self.leases.get_mut(lease_id) {
                    // Only renew if lease hasn't expired
                    if lease.expires_at > *timestamp {
                        lease.expires_at =
                            timestamp.saturating_add(lease.ttl_secs.saturating_mul(1_000_000_000));
                    }
                }
            }

            OwnershipCommand::ReleaseLease { key } => {
                if let Some(lease_id) = self.key_leases.remove(key) {
                    self.leases.remove(&lease_id);
                }
            }

            OwnershipCommand::ExpireLeases { now } => {
                let expired_keys: Vec<String> = self
                    .leases
                    .values()
                    .filter(|lease| lease.expires_at <= *now)
                    .map(|lease| lease.key.clone())
                    .collect();

                for key in expired_keys {
                    self.key_leases.remove(&key);
                    self.leases.retain(|_, lease| lease.key != key);
                }
            }
        }
    }

    /// Query the state machine
    pub fn query(&self, query: &OwnershipQuery) -> OwnershipResponse {
        match query {
            OwnershipQuery::GetOwner { topic } => OwnershipResponse::Owner {
                topic: topic.clone(),
                owner: self.ownership.get(topic).copied(),
            },

            OwnershipQuery::ListTopics { node_id } => OwnershipResponse::Topics {
                topics: self.node_topics.get(node_id).cloned().unwrap_or_default(),
            },

            OwnershipQuery::ListAll => OwnershipResponse::All {
                ownership: self.ownership.clone(),
            },

            // ==================================================================
            // Lease Queries
            // ==================================================================
            OwnershipQuery::GetLeaseOwner { key } => {
                let info = self.get_lease_info(key);
                OwnershipResponse::LeaseOwner {
                    key: key.clone(),
                    info,
                }
            }

            OwnershipQuery::ListLeases { owner } => OwnershipResponse::Leases {
                leases: self.list_leases(*owner),
            },

            OwnershipQuery::GetLease { lease_id } => OwnershipResponse::LeaseResult {
                lease: self.leases.get(lease_id).cloned(),
                error: None,
            },
        }
    }

    /// Get owner of a topic (convenience method)
    pub fn get_owner(&self, topic: &str) -> Option<NodeId> {
        self.ownership.get(topic).copied()
    }

    /// Check if a node owns a topic
    pub fn owns(&self, topic: &str, node_id: NodeId) -> bool {
        self.ownership.get(topic) == Some(&node_id)
    }

    /// Get all topics owned by a node
    pub fn get_node_topics(&self, node_id: NodeId) -> Vec<String> {
        self.node_topics.get(&node_id).cloned().unwrap_or_default()
    }

    /// Get total number of topics with ownership
    pub fn topic_count(&self) -> usize {
        self.ownership.len()
    }

    // ========================================================================
    // Lease Helper Methods
    // ========================================================================

    /// Get lease info for a key (checks expiration)
    pub fn get_lease_info(&self, key: &str) -> Option<LeaseInfo> {
        let lease_id = self.key_leases.get(key)?;
        let lease = self.leases.get(lease_id)?;
        Some(LeaseInfo::from(lease.clone()))
    }

    /// List all leases for a node
    pub fn list_leases(&self, owner: NodeId) -> Vec<LeaseInfo> {
        self.leases
            .values()
            .filter(|lease| lease.owner == owner)
            .map(|lease| LeaseInfo::from(lease.clone()))
            .collect()
    }

    /// Check if a lease is valid (not expired)
    pub fn is_lease_valid(&self, lease_id: u64, now: u64) -> bool {
        self.leases
            .get(&lease_id)
            .map(|lease| lease.expires_at > now)
            .unwrap_or(false)
    }

    /// Expire stale leases and return count
    pub fn expire_leases(&mut self, now: u64) -> usize {
        let expired_keys: Vec<String> = self
            .leases
            .values()
            .filter(|lease| lease.expires_at <= now)
            .map(|lease| lease.key.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.key_leases.remove(&key);
            self.leases.retain(|_, lease| lease.key != key);
        }
        count
    }

    /// Get lease entry by ID
    pub fn get_lease(&self, lease_id: u64) -> Option<&LeaseEntry> {
        self.leases.get(&lease_id)
    }

    /// Get total number of active leases
    pub fn lease_count(&self) -> usize {
        self.leases.len()
    }

    /// Generate next lease ID
    pub fn next_lease_id(&mut self) -> u64 {
        let id = self.next_lease_id;
        self.next_lease_id += 1;
        id
    }

    /// Acquire a lease programmatically (with auto-generated ID)
    /// Returns the lease ID if successful
    pub fn acquire_lease(
        &mut self,
        key: String,
        owner: NodeId,
        ttl_secs: u64,
        timestamp: u64,
    ) -> Result<u64, LeaseError> {
        // Check if key already has a valid lease
        if let Some(existing_id) = self.key_leases.get(&key) {
            if let Some(existing) = self.leases.get(existing_id) {
                if existing.expires_at > timestamp {
                    return Err(LeaseError::LeaseHeld {
                        key,
                        owner: existing.owner,
                    });
                }
            }
        }

        let lease_id = self.next_lease_id();
        let expires_at = timestamp.saturating_add(ttl_secs.saturating_mul(1_000_000_000));

        let lease = LeaseEntry {
            id: lease_id,
            key: key.clone(),
            owner,
            acquired_at: timestamp,
            ttl_secs,
            expires_at,
        };

        self.leases.insert(lease_id, lease);
        self.key_leases.insert(key, lease_id);

        Ok(lease_id)
    }
}

impl Validate for OwnershipState {
    fn validate(&self) -> Result<(), super::lock_utils::ValidationError> {
        use super::lock_utils::ValidationError;

        // Check lease consistency: every key_lease should have a valid lease
        for (key, lease_id) in &self.key_leases {
            if !self.leases.contains_key(lease_id) {
                tracing::error!(
                    key = %key,
                    lease_id = %lease_id,
                    "Orphaned key_lease: key references non-existent lease"
                );
                return Err(ValidationError::OrphanedKeyLease);
            }
        }

        // Check lease consistency: every lease should be in key_leases
        for (lease_id, lease) in &self.leases {
            if let Some(mapped_id) = self.key_leases.get(&lease.key) {
                if *mapped_id != *lease_id {
                    tracing::error!(
                        lease_id = %lease_id,
                        mapped_id = %mapped_id,
                        key = %lease.key,
                        "Lease ID inconsistency in key_leases"
                    );
                    return Err(ValidationError::LeaseIdInconsistency);
                }
            } else {
                tracing::error!(
                    lease_id = %lease_id,
                    key = %lease.key,
                    "Dangling lease: lease not tracked in key_leases"
                );
                return Err(ValidationError::DanglingLease);
            }
        }

        // Check lease expiration consistency
        for (_, lease) in &self.leases {
            let expected_expires = lease
                .acquired_at
                .saturating_add(lease.ttl_secs.saturating_mul(1_000_000_000));
            if lease.expires_at != expected_expires {
                tracing::error!(
                    lease_id = %lease.id,
                    expires_at = %lease.expires_at,
                    expected = %expected_expires,
                    "Invalid lease expiration time"
                );
                return Err(ValidationError::InvalidLeaseExpiration);
            }
        }

        // Check ownership consistency: topics in ownership should be in node_topics
        for (topic, owner) in &self.ownership {
            if let Some(topics) = self.node_topics.get(owner) {
                if !topics.contains(topic) {
                    tracing::error!(
                        topic = %topic,
                        owner = %owner,
                        "Ownership inconsistency: topic not in owner's topic list"
                    );
                    // This is a custom check, not in ValidationError enum yet
                    return Err(ValidationError::Custom(format!(
                        "Topic {} owned by {} not in owner's topic list",
                        topic, owner
                    )));
                }
            } else {
                tracing::error!(
                    topic = %topic,
                    owner = %owner,
                    "Ownership inconsistency: owner has no topic list"
                );
                return Err(ValidationError::Custom(format!(
                    "Owner {} has no topic list but owns topic {}",
                    owner, topic
                )));
            }
        }

        // Check node_topics consistency: all topics should be in ownership
        for (node_id, topics) in &self.node_topics {
            for topic in topics {
                match self.ownership.get(topic) {
                    Some(owner) if *owner == *node_id => {
                        // Correct ownership
                    }
                    Some(owner) => {
                        tracing::error!(
                            topic = %topic,
                            expected_owner = %node_id,
                            actual_owner = %owner,
                            "Node topics inconsistency: topic owned by different node"
                        );
                        return Err(ValidationError::Custom(format!(
                            "Topic {} in node {}'s list but owned by {}",
                            topic, node_id, owner
                        )));
                    }
                    None => {
                        tracing::error!(
                            topic = %topic,
                            node_id = %node_id,
                            "Node topics inconsistency: topic has no owner"
                        );
                        return Err(ValidationError::Custom(format!(
                            "Topic {} in node {}'s list but has no owner",
                            topic, node_id
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claim_topic() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        assert_eq!(state.get_owner("metrics.cpu"), Some(1));
        assert!(state.owns("metrics.cpu", 1));
        assert!(!state.owns("metrics.cpu", 2));
    }

    #[test]
    fn test_claim_transfers_ownership() {
        let mut state = OwnershipState::new();

        // Node 1 claims
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
        assert_eq!(state.get_owner("metrics.cpu"), Some(1));

        // Node 2 claims same topic - should transfer
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 2,
            timestamp: 2000,
        });
        assert_eq!(state.get_owner("metrics.cpu"), Some(2));
        assert!(!state.owns("metrics.cpu", 1));
        assert!(state.owns("metrics.cpu", 2));
    }

    #[test]
    fn test_release_topic() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        // Non-owner trying to release should do nothing
        state.apply(&OwnershipCommand::ReleaseTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 2,
        });
        assert_eq!(state.get_owner("metrics.cpu"), Some(1));

        // Owner releasing should work
        state.apply(&OwnershipCommand::ReleaseTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
        });
        assert_eq!(state.get_owner("metrics.cpu"), None);
    }

    #[test]
    fn test_transfer_topic() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        // Transfer to node 2
        state.apply(&OwnershipCommand::TransferTopic {
            topic: "metrics.cpu".to_string(),
            from_node: 1,
            to_node: 2,
        });

        assert_eq!(state.get_owner("metrics.cpu"), Some(2));
        assert!(state.owns("metrics.cpu", 2));
        assert!(!state.owns("metrics.cpu", 1));
    }

    #[test]
    fn test_transfer_by_non_owner_fails() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        // Node 2 (non-owner) tries to transfer - should fail
        state.apply(&OwnershipCommand::TransferTopic {
            topic: "metrics.cpu".to_string(),
            from_node: 2,
            to_node: 3,
        });

        // Ownership should remain with node 1
        assert_eq!(state.get_owner("metrics.cpu"), Some(1));
    }

    #[test]
    fn test_query_get_owner() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        let response = state.query(&OwnershipQuery::GetOwner {
            topic: "metrics.cpu".to_string(),
        });

        match response {
            OwnershipResponse::Owner { topic, owner } => {
                assert_eq!(topic, "metrics.cpu");
                assert_eq!(owner, Some(1));
            }
            _ => panic!("Expected Owner response"),
        }
    }

    #[test]
    fn test_query_list_topics() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.mem".to_string(),
            node_id: 1,
            timestamp: 1001,
        });

        let response = state.query(&OwnershipQuery::ListTopics { node_id: 1 });

        match response {
            OwnershipResponse::Topics { topics } => {
                assert_eq!(topics.len(), 2);
                assert!(topics.contains(&"metrics.cpu".to_string()));
                assert!(topics.contains(&"metrics.mem".to_string()));
            }
            _ => panic!("Expected Topics response"),
        }
    }

    #[test]
    fn test_query_list_all() {
        let mut state = OwnershipState::new();

        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "logs.app".to_string(),
            node_id: 2,
            timestamp: 1001,
        });

        let response = state.query(&OwnershipQuery::ListAll);

        match response {
            OwnershipResponse::All { ownership } => {
                assert_eq!(ownership.len(), 2);
                assert_eq!(ownership.get("metrics.cpu"), Some(&1));
                assert_eq!(ownership.get("logs.app"), Some(&2));
            }
            _ => panic!("Expected All response"),
        }
    }

    #[test]
    fn test_multiple_topics_same_node() {
        let mut state = OwnershipState::new();

        // Node 1 claims multiple topics
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.mem".to_string(),
            node_id: 1,
            timestamp: 1001,
        });
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.disk".to_string(),
            node_id: 1,
            timestamp: 1002,
        });

        let topics = state.get_node_topics(1);
        assert_eq!(topics.len(), 3);
        assert_eq!(state.topic_count(), 3);
    }

    // ========================================================================
    // Lease Tests
    // ========================================================================

    #[test]
    fn test_acquire_lease() {
        let mut state = OwnershipState::new();

        // Acquire a lease
        let key = "shard-0".to_string();
        let lease_id = state.acquire_lease(key.clone(), 1, 10, 1000).unwrap();

        assert_eq!(lease_id, 0); // First lease ID
        assert_eq!(state.lease_count(), 1);

        // Get lease info
        let info = state.get_lease_info(&key).unwrap();
        assert_eq!(info.key, key);
        assert_eq!(info.owner, 1);
        assert_eq!(info.acquired_at, 1000);
        // expires_at = 1000 + 10 * 1_000_000_000 = 10_000_000_1000
        assert_eq!(info.expires_at, 1000 + 10_000_000_000);
    }

    #[test]
    fn test_lease_conflict() {
        let mut state = OwnershipState::new();

        let key = "shard-0".to_string();
        let _ = state.acquire_lease(key.clone(), 1, 10, 1000).unwrap();

        // Try to acquire same key before expiration (same owner should fail too)
        let result = state.acquire_lease(key.clone(), 1, 10, 2000);
        assert!(matches!(result, Err(LeaseError::LeaseHeld { .. })));

        // Try with different owner
        let result = state.acquire_lease(key.clone(), 2, 10, 2000);
        assert!(matches!(
            result,
            Err(LeaseError::LeaseHeld { owner: 1, .. })
        ));
    }

    #[test]
    fn test_lease_expiration() {
        let mut state = OwnershipState::new();

        let key = "shard-0".to_string();
        // TTL=10 seconds, acquired at t=1000, expires at t=10_000_000_1000
        let _ = state.acquire_lease(key.clone(), 1, 10, 1000).unwrap();

        // Expire at time before expiration
        let count = state.expire_leases(5000);
        assert_eq!(count, 0);
        assert_eq!(state.lease_count(), 1);

        // Expire at time after expiration
        let count = state.expire_leases(10_000_000_2000);
        assert_eq!(count, 1);
        assert_eq!(state.lease_count(), 0);

        // Can now acquire the key again
        let result = state.acquire_lease(key.clone(), 2, 10, 10_000_000_2000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lease_renewal() {
        let mut state = OwnershipState::new();

        // Create lease via command (to test renew)
        state.apply(&OwnershipCommand::AcquireLease {
            key: "shard-0".to_string(),
            owner: 1,
            lease_id: 42,
            ttl_secs: 10,
            timestamp: 1000,
        });

        // Check initial expiration
        let lease = state.get_lease(42).unwrap();
        assert_eq!(lease.expires_at, 1000 + 10_000_000_000);

        // Renew the lease
        state.apply(&OwnershipCommand::RenewLease {
            lease_id: 42,
            timestamp: 5_000_000_000,
        });

        // Check renewed expiration
        let lease = state.get_lease(42).unwrap();
        assert_eq!(lease.expires_at, 5_000_000_000 + 10_000_000_000);
    }

    #[test]
    fn test_lease_release() {
        let mut state = OwnershipState::new();

        // Create lease via command
        state.apply(&OwnershipCommand::AcquireLease {
            key: "shard-0".to_string(),
            owner: 1,
            lease_id: 42,
            ttl_secs: 10,
            timestamp: 1000,
        });

        assert_eq!(state.lease_count(), 1);

        // Release the lease
        state.apply(&OwnershipCommand::ReleaseLease {
            key: "shard-0".to_string(),
        });

        assert_eq!(state.lease_count(), 0);
        assert!(state.get_lease_info("shard-0").is_none());
    }

    #[test]
    fn test_lease_commands_integration() {
        let mut state = OwnershipState::new();

        // Acquire via command
        state.apply(&OwnershipCommand::AcquireLease {
            key: "shard-0".to_string(),
            owner: 1,
            lease_id: 100,
            ttl_secs: 60,
            timestamp: 1000,
        });

        // Query lease owner
        let response = state.query(&OwnershipQuery::GetLeaseOwner {
            key: "shard-0".to_string(),
        });

        match response {
            OwnershipResponse::LeaseOwner { key, info } => {
                assert_eq!(key, "shard-0");
                let info = info.unwrap();
                assert_eq!(info.owner, 1);
            }
            _ => panic!("Expected LeaseOwner response"),
        }

        // List leases for node 1
        let response = state.query(&OwnershipQuery::ListLeases { owner: 1 });

        match response {
            OwnershipResponse::Leases { leases } => {
                assert_eq!(leases.len(), 1);
                assert_eq!(leases[0].key, "shard-0");
            }
            _ => panic!("Expected Leases response"),
        }

        // Get lease by ID
        let response = state.query(&OwnershipQuery::GetLease { lease_id: 100 });

        match response {
            OwnershipResponse::LeaseResult { lease, error } => {
                assert!(error.is_none());
                let lease = lease.unwrap();
                assert_eq!(lease.key, "shard-0");
                assert_eq!(lease.owner, 1);
            }
            _ => panic!("Expected LeaseResult response"),
        }
    }

    #[test]
    fn test_next_lease_id_monotonic() {
        let mut state = OwnershipState::new();

        let id1 = state.next_lease_id();
        let id2 = state.next_lease_id();
        let id3 = state.next_lease_id();

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[test]
    fn test_acquire_lease_command_updates_next_id() {
        let mut state = OwnershipState::new();

        // Apply with explicit ID
        state.apply(&OwnershipCommand::AcquireLease {
            key: "shard-0".to_string(),
            owner: 1,
            lease_id: 100, // Explicit high ID
            ttl_secs: 10,
            timestamp: 1000,
        });

        // Next ID should be 101
        assert_eq!(state.next_lease_id, 101);
    }

    #[test]
    fn test_is_lease_valid() {
        let mut state = OwnershipState::new();

        let _ = state
            .acquire_lease("shard-0".to_string(), 1, 10, 1000)
            .unwrap();
        let lease_id = 0; // First lease

        // Valid before expiration
        assert!(state.is_lease_valid(lease_id, 5000));

        // Invalid after expiration
        assert!(!state.is_lease_valid(lease_id, 10_000_000_1001));

        // Non-existent lease
        assert!(!state.is_lease_valid(99999, 1000));
    }
}
