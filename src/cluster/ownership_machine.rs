//! Ownership state machine for Raft
//!
//! Provides strongly consistent topic ownership via Raft consensus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique node identifier
pub type NodeId = u64;

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
}

/// Ownership state (the state machine)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OwnershipState {
    /// topic -> owner_node_id
    ownership: HashMap<String, NodeId>,
    /// node_id -> topics (reverse index for fast lookup)
    node_topics: HashMap<NodeId, Vec<String>>,
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
}
