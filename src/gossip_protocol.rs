//! Gossip protocol messages for DDL topic ownership
//!
//! Uses iroh-gossip for epidemic broadcast of topic ownership claims.

use serde::{Deserialize, Serialize};

/// Unique node identifier (derived from iroh node ID hash)
pub type NodeId = u64;

/// Topic identifier (32 bytes, BLAKE3 hash of topic name)
pub type TopicId = [u8; 32];

/// Default epoch value for backwards compatibility with old nodes
fn default_epoch() -> u64 {
    1
}

/// Gossip message types for DDL coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Claim ownership of a topic
    TopicClaim {
        topic: String,
        topic_id: TopicId,
        node_id: NodeId,
        /// Epoch of this claim (incremented on each ownership change)
        #[serde(default = "default_epoch")]
        epoch: u64,
        timestamp: u64,
    },

    /// Release ownership of a topic
    TopicRelease {
        topic: String,
        topic_id: TopicId,
        node_id: NodeId,
    },

    /// Heartbeat to maintain ownership claim
    TopicHeartbeat {
        topic: String,
        topic_id: TopicId,
        node_id: NodeId,
        /// Epoch at time of heartbeat
        #[serde(default = "default_epoch")]
        epoch: u64,
        timestamp: u64,
    },

    /// Node joining the mesh
    NodeAnnounce {
        node_id: NodeId,
        bind_addr: String,
        owned_topics: Vec<String>,
    },

    /// Node leaving the mesh
    NodeLeave { node_id: NodeId },

    /// Query for topic owner
    TopicQuery {
        topic: String,
        requestor_node_id: NodeId,
    },

    /// Response to topic query
    TopicResponse {
        topic: String,
        owner_node_id: Option<NodeId>,
    },
}

/// Topic ownership record
#[derive(Debug, Clone)]
pub struct TopicOwnership {
    /// Node that owns this topic
    pub owner_node_id: NodeId,
    /// Epoch incremented on each ownership change (starts at 1)
    pub epoch: u64,
    /// When ownership was claimed (nanoseconds since epoch)
    pub claimed_at: u64,
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
}

impl TopicOwnership {
    /// Create a new ownership record with epoch starting at 1
    pub fn new(owner_node_id: NodeId) -> Self {
        let now = crate::types::now_nanos();
        Self {
            owner_node_id,
            epoch: 1,
            claimed_at: now,
            last_heartbeat: now,
        }
    }

    /// Update heartbeat timestamp
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = crate::types::now_nanos();
    }

    /// Check if ownership has timed out (no heartbeat for specified duration)
    pub fn is_timed_out(&self, timeout_nanos: u64) -> bool {
        let now = crate::types::now_nanos();
        now.saturating_sub(self.last_heartbeat) > timeout_nanos
    }

    /// Transfer ownership to a new node with epoch increment
    pub fn transfer_to(&mut self, new_owner: NodeId) {
        self.owner_node_id = new_owner;
        self.epoch = self.epoch.saturating_add(1);
        self.claimed_at = crate::types::now_nanos();
        self.last_heartbeat = crate::types::now_nanos();
    }
}

/// Derive topic ID from topic name using BLAKE3
pub fn derive_topic_id(topic: &str) -> TopicId {
    blake3::hash(topic.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_topic_id() {
        let id1 = derive_topic_id("test.topic");
        let id2 = derive_topic_id("test.topic");
        let id3 = derive_topic_id("other.topic");

        // Same topic should produce same ID
        assert_eq!(id1, id2);
        // Different topics should produce different IDs
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_topic_ownership_timeout() {
        let mut ownership = TopicOwnership::new(1);
        assert!(!ownership.is_timed_out(1_000_000_000)); // 1 second timeout

        // Simulate time passing by manually setting last_heartbeat
        ownership.last_heartbeat = crate::types::now_nanos() - 2_000_000_000; // 2 seconds ago
        assert!(ownership.is_timed_out(1_000_000_000));
    }

    #[test]
    fn test_ownership_heartbeat_update() {
        let mut ownership = TopicOwnership::new(1);
        let old_heartbeat = ownership.last_heartbeat;

        // Small sleep to ensure time passes
        std::thread::sleep(std::time::Duration::from_millis(1));
        ownership.update_heartbeat();

        assert!(ownership.last_heartbeat > old_heartbeat);
    }

    #[test]
    fn test_ownership_epoch_starts_at_one() {
        let ownership = TopicOwnership::new(1);
        assert_eq!(ownership.epoch, 1);
    }

    #[test]
    fn test_ownership_transfer_increments_epoch() {
        let mut ownership = TopicOwnership::new(1);
        assert_eq!(ownership.epoch, 1);
        assert_eq!(ownership.owner_node_id, 1);

        // Transfer to node 2
        ownership.transfer_to(2);
        assert_eq!(ownership.epoch, 2);
        assert_eq!(ownership.owner_node_id, 2);

        // Transfer again to node 3
        ownership.transfer_to(3);
        assert_eq!(ownership.epoch, 3);
        assert_eq!(ownership.owner_node_id, 3);
    }

    #[test]
    fn test_message_serialization() {
        let msg = GossipMessage::TopicClaim {
            topic: "test.topic".to_string(),
            topic_id: derive_topic_id("test.topic"),
            node_id: 42,
            epoch: 1,
            timestamp: crate::types::now_nanos(),
        };

        // Should serialize and deserialize correctly
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: GossipMessage = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            GossipMessage::TopicClaim {
                topic,
                node_id,
                epoch,
                ..
            } => {
                assert_eq!(topic, "test.topic");
                assert_eq!(node_id, 42);
                assert_eq!(epoch, 1);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_backwards_compatibility() {
        // Old message format (without epoch field) should default to epoch=1
        let old_format = r#"{"TopicClaim":{"topic":"test.topic","topic_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"node_id":42,"timestamp":1234567890}}"#;
        let deserialized: GossipMessage = serde_json::from_str(old_format)
            .expect("Should deserialize old format without epoch field");

        match deserialized {
            GossipMessage::TopicClaim { epoch, .. } => {
                assert_eq!(epoch, 1, "Should default to epoch=1 for old format");
            }
            _ => panic!("Wrong message type"),
        }

        // Test TopicHeartbeat backwards compatibility
        let old_heartbeat = r#"{"TopicHeartbeat":{"topic":"test.topic","topic_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"node_id":42,"timestamp":1234567890}}"#;
        let deserialized: GossipMessage =
            serde_json::from_str(old_heartbeat).expect("Should deserialize old heartbeat format");

        match deserialized {
            GossipMessage::TopicHeartbeat { epoch, .. } => {
                assert_eq!(
                    epoch, 1,
                    "Should default to epoch=1 for old heartbeat format"
                );
            }
            _ => panic!("Wrong message type"),
        }
    }
}
