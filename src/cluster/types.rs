//! Raft types for AutoQueues cluster coordination
//!
//! Defines the core types for Raft-based topic ownership.

use openraft::impls::{BasicNode, Entry, TokioRuntime};
use openraft::{CommittedLeaderId, LogId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cluster::ownership_machine::{OwnershipCommand, OwnershipResponse};

#[derive(Debug, Clone, Serialize, Deserialize, Default, Eq, PartialEq)]
pub struct NodeConfig {
    pub node_id: u64,
    pub host: String,
    pub communication_port: u16,
    pub coordination_port: u16,
}

/// Legacy entry data for backwards compatibility with existing code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryData {
    pub metric_name: String,
    pub aggregation_type: String,
    pub value: f64,
    pub timestamp: u64,
    pub sources: HashMap<String, f64>,
}

/// Serializable wrapper for Raft log entries
/// OpenRaft's Entry<TypeConfig> doesn't implement Serialize/Deserialize,
/// so we need this wrapper for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableLogEntry {
    pub index: u64,
    pub term: u64,
    pub leader_id: u64,
    pub command: Option<OwnershipCommand>,
}

impl From<&Entry<TypeConfig>> for SerializableLogEntry {
    fn from(entry: &Entry<TypeConfig>) -> Self {
        Self {
            index: entry.log_id.index,
            term: entry.log_id.leader_id.term,
            leader_id: entry.log_id.leader_id.node_id,
            command: match &entry.payload {
                openraft::entry::EntryPayload::Normal(cmd) => Some(cmd.clone()),
                _ => None,
            },
        }
    }
}

impl From<SerializableLogEntry> for Entry<TypeConfig> {
    fn from(se: SerializableLogEntry) -> Self {
        Entry {
            log_id: LogId::new(CommittedLeaderId::new(se.term, se.leader_id), se.index),
            payload: match se.command {
                Some(cmd) => openraft::entry::EntryPayload::Normal(cmd),
                None => openraft::entry::EntryPayload::Blank,
            },
        }
    }
}

/// Serializable wrapper for Raft vote
/// OpenRaft's Vote<u64> doesn't implement Serialize/Deserialize,
/// so we need this wrapper for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableVote {
    pub term: u64,
    pub node_id: u64,
    pub committed: bool,
}

openraft::declare_raft_types!(
    pub TypeConfig:
        D            = OwnershipCommand,
        R            = OwnershipResponse,
        NodeId       = u64,
        Node         = BasicNode,
        Entry        = Entry<TypeConfig>,
        SnapshotData = std::io::Cursor<Vec<u8>>,
        AsyncRuntime = TokioRuntime,
);

impl std::fmt::Display for NodeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeConfig(id={}, {}:{{comm={}, coord={}}})",
            self.node_id, self.host, self.communication_port, self.coordination_port
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_display() {
        let config = NodeConfig {
            node_id: 1,
            host: "localhost".to_string(),
            communication_port: 8080,
            coordination_port: 9090,
        };

        let display = format!("{}", config);
        assert!(display.contains("id=1") || display.contains("node_id=1"));
        assert!(display.contains("localhost"));
    }

    #[test]
    fn test_ownership_command_serialization() {
        let cmd = OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: OwnershipCommand = serde_json::from_str(&json).unwrap();

        match deserialized {
            OwnershipCommand::ClaimTopic { topic, node_id, .. } => {
                assert_eq!(topic, "metrics.cpu");
                assert_eq!(node_id, 1);
            }
            _ => panic!("Expected ClaimTopic"),
        }
    }

    #[test]
    fn test_ownership_response_serialization() {
        let response = OwnershipResponse::Owner {
            topic: "metrics.cpu".to_string(),
            owner: Some(1),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: OwnershipResponse = serde_json::from_str(&json).unwrap();

        match deserialized {
            OwnershipResponse::Owner { topic, owner } => {
                assert_eq!(topic, "metrics.cpu");
                assert_eq!(owner, Some(1));
            }
            _ => panic!("Expected Owner response"),
        }
    }
}
