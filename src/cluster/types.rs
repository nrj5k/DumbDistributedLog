use openraft::impls::{BasicNode, Entry, TokioRuntime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default, Eq, PartialEq)]
pub struct NodeConfig {
    pub node_id: u64,
    pub host: String,
    pub communication_port: u16,
    pub coordination_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryData {
    pub metric_name: String,
    pub aggregation_type: String,
    pub value: f64,
    pub timestamp: u64,
    pub sources: HashMap<String, f64>,
}

openraft::declare_raft_types!(
    pub TypeConfig:
        D            = EntryData,
        R            = EntryData,
        NodeId       = u64,
        Node         = BasicNode,
        Entry        = Entry<TypeConfig>,
        SnapshotData = std::io::Cursor<Vec<u8>>,
        AsyncRuntime = TokioRuntime,
);
