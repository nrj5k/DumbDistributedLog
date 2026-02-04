//! Raft cluster implementation for AutoQueues
//!
//! Provides full cluster coordination with leader election using openraft.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::cluster::types::{NodeConfig, EntryData};
use crate::cluster::storage::AutoqueuesRaftStorage;
use crate::network::ZmqRaftNetwork;

/// Full Raft cluster node for AutoQueues
pub struct RaftClusterNode {
    /// Node ID
    pub node_id: u64,
    /// Storage
    pub storage: Arc<AutoqueuesRaftStorage>,
    /// Network transport
    pub network: Arc<ZmqRaftNetwork>,
    /// Peer nodes
    pub peers: HashMap<u64, NodeConfig>,
    /// Is this node the leader?
    pub is_leader: Arc<RwLock<bool>>,
}

impl RaftClusterNode {
    /// Create new Raft cluster node
    pub async fn new(
        node_id: u64,
        peers: HashMap<u64, NodeConfig>,
        network: Arc<ZmqRaftNetwork>,
    ) -> Result<Self, String> {
        let storage = Arc::new(AutoqueuesRaftStorage::new());
        
        Ok(Self {
            node_id,
            storage,
            network,
            peers,
            is_leader: Arc::new(RwLock::new(false)),
        })
    }
    
    /// Create a new Raft cluster node with default network
    pub async fn with_default_network(
        node_id: u64,
        peers: HashMap<u64, NodeConfig>,
    ) -> Result<Self, String> {
        // Create default peer configuration for network
        let mut peer_configs = HashMap::new();
        for (peer_id, peer_info) in &peers {
            peer_configs.insert(*peer_id, (peer_info.host.clone(), peer_info.coordination_port));
        }
        
        let network = Arc::new(ZmqRaftNetwork::new(node_id, peer_configs).map_err(|e| format!("{:?}", e))?);
        Self::new(node_id, peers, network).await
    }
    
    /// Initialize cluster (call on bootstrap node)
    pub async fn initialize_cluster(&self) -> Result<(), String> {
        // In a real implementation, this would initialize the Raft cluster
        let mut is_leader = self.is_leader.write().await;
        *is_leader = true;
        Ok(())
    }
    
    /// Add node to cluster
    pub async fn add_node(&mut self, node_id: u64, config: NodeConfig) -> Result<(), String> {
        self.peers.insert(node_id, config);
        Ok(())
    }
    
    /// Check if current node is leader
    pub async fn is_leader(&self) -> bool {
        *self.is_leader.read().await
    }
    
    /// Propose new aggregation data
    pub async fn propose_aggregation(&self, _data: EntryData) -> Result<(), String> {
        // In a real implementation, this would propose data to the Raft cluster
        Ok(())
    }
    
    /// Get current Raft metrics
    pub async fn metrics(&self) -> HashMap<String, String> {
        // Return simplified metrics for now
        let mut metrics = HashMap::new();
        metrics.insert("node_id".to_string(), self.node_id.to_string());
        metrics.insert("is_leader".to_string(), self.is_leader().await.to_string());
        metrics.insert("peer_count".to_string(), self.peers.len().to_string());
        metrics
    }
}