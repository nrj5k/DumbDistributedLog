//! Membership tracking and events for distributed coordination.
//!
//! Provides node lifecycle events for SCORE failover coordination.

use std::collections::HashMap;
use std::time::SystemTime;

/// Membership event types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipEventType {
    /// A new node joined the cluster
    NodeJoined { node_id: u64, addr: String },
    /// A node gracefully left the cluster
    NodeLeft { node_id: u64 },
    /// A node failed (crashed or network partition)
    NodeFailed { node_id: u64 },
    /// A node recovered after failure
    NodeRecovered { node_id: u64 },
}

/// Membership event emitted when cluster membership changes
#[derive(Debug, Clone)]
pub struct MembershipEvent {
    /// Type of event
    pub event_type: MembershipEventType,
    /// When the event occurred
    pub timestamp: SystemTime,
}

/// Current view of cluster membership
#[derive(Debug, Clone)]
pub struct MembershipView {
    /// All known nodes in the cluster
    pub nodes: HashMap<u64, NodeInfo>,
    /// Current Raft leader (if any)
    pub leader: Option<u64>,
    /// This node's ID
    pub local_node_id: u64,
}

/// Information about a cluster node
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node identifier
    pub node_id: u64,
    /// Network address
    pub addr: String,
    /// Last seen time (for failed detection)
    pub last_seen: SystemTime,
    /// Is this node the leader?
    pub is_leader: bool,
}

/// DDL metrics for monitoring
#[derive(Debug, Clone)]
pub struct DdlMetrics {
    // Membership
    /// Number of active leases
    pub active_leases: usize,
    /// Size of membership
    pub membership_size: usize,

    // State
    /// Size of state in bytes
    pub state_size_bytes: usize,
    /// Number of keys in state
    pub state_key_count: usize,

    // Raft
    /// Raft commit index
    pub raft_commit_index: u64,
    /// Raft applied index
    pub raft_applied_index: u64,
    /// Current Raft leader
    pub raft_leader: Option<u64>,

    // Network
    /// Number of pending writes
    pub pending_writes: usize,
    /// Number of pending reads
    pub pending_reads: usize,
}

impl MembershipEvent {
    /// Create a new membership event
    pub fn new(event_type: MembershipEventType) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
        }
    }

    /// Create NodeJoined event
    pub fn node_joined(node_id: u64, addr: String) -> Self {
        Self::new(MembershipEventType::NodeJoined { node_id, addr })
    }

    /// Create NodeLeft event
    pub fn node_left(node_id: u64) -> Self {
        Self::new(MembershipEventType::NodeLeft { node_id })
    }

    /// Create NodeFailed event
    pub fn node_failed(node_id: u64) -> Self {
        Self::new(MembershipEventType::NodeFailed { node_id })
    }

    /// Create NodeRecovered event
    pub fn node_recovered(node_id: u64) -> Self {
        Self::new(MembershipEventType::NodeRecovered { node_id })
    }
}

impl std::fmt::Display for MembershipEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeJoined { node_id, .. } => write!(f, "NodeJoined({})", node_id),
            Self::NodeLeft { node_id } => write!(f, "NodeLeft({})", node_id),
            Self::NodeFailed { node_id } => write!(f, "NodeFailed({})", node_id),
            Self::NodeRecovered { node_id } => write!(f, "NodeRecovered({})", node_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_membership_event_creation() {
        let event = MembershipEvent::node_joined(1, "localhost:9090".to_string());
        assert!(matches!(
            event.event_type,
            MembershipEventType::NodeJoined { node_id: 1, .. }
        ));
    }

    #[test]
    fn test_membership_event_node_left() {
        let event = MembershipEvent::node_left(1);
        assert!(matches!(
            event.event_type,
            MembershipEventType::NodeLeft { node_id: 1 }
        ));
    }

    #[test]
    fn test_membership_event_node_failed() {
        let event = MembershipEvent::node_failed(1);
        assert!(matches!(
            event.event_type,
            MembershipEventType::NodeFailed { node_id: 1 }
        ));
    }

    #[test]
    fn test_membership_event_node_recovered() {
        let event = MembershipEvent::node_recovered(1);
        assert!(matches!(
            event.event_type,
            MembershipEventType::NodeRecovered { node_id: 1 }
        ));
    }

    #[test]
    fn test_event_type_display() {
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

    #[test]
    fn test_timestamp_set() {
        let before = SystemTime::now();
        let event = MembershipEvent::node_joined(1, "localhost:9090".to_string());
        let after = SystemTime::now();

        assert!(event.timestamp >= before);
        assert!(event.timestamp <= after);
    }

    #[test]
    fn test_membership_view_creation() {
        let view = MembershipView {
            nodes: HashMap::new(),
            leader: Some(1),
            local_node_id: 1,
        };
        assert_eq!(view.local_node_id, 1);
        assert_eq!(view.leader, Some(1));
        assert_eq!(view.nodes.len(), 0);
    }

    #[test]
    fn test_node_info_creation() {
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

    #[test]
    fn test_ddl_metrics_defaults() {
        let metrics = DdlMetrics {
            active_leases: 0,
            membership_size: 5,
            state_size_bytes: 1024,
            state_key_count: 100,
            raft_commit_index: 42,
            raft_applied_index: 40,
            raft_leader: Some(1),
            pending_writes: 10,
            pending_reads: 5,
        };
        assert_eq!(metrics.membership_size, 5);
        assert_eq!(metrics.raft_commit_index, 42);
        assert_eq!(metrics.raft_leader, Some(1));
    }
}
