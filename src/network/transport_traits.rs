//! Transport traits for AutoQueues - HPC-Optimized
//!
//! Minimal traits for high-performance networking with sub-ms latency targets.
//! Designed for RDMA compatibility and zero-copy operations.

use std::time::Instant;

// Re-export the canonical TransportError from tcp.rs
pub use crate::network::tcp::TransportError;

/// Transport type enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    /// RDMA transport for ultra-low latency
    Rdma,

    /// TCP transport for compatibility
    Tcp,

    /// In-memory transport for local testing
    InMemory,
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Remote address
    pub remote_addr: String,

    /// Transport type
    pub transport_type: TransportType,

    /// Connection timestamp
    pub connected_at: Instant,
}

/// Minimal transport trait for HPC use cases
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Connect to remote address
    async fn connect(&mut self, addr: &str) -> Result<ConnectionInfo, TransportError>;

    /// Send data with zero-copy semantics
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Receive data with zero-copy semantics
    async fn receive(&mut self) -> Result<Vec<u8>, TransportError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;

    /// Get connection information
    fn connection_info(&self) -> Option<ConnectionInfo>;
}

/// Raft network connection trait for cluster coordination
///
/// NOTE: Full implementation will be done in Phase 2.
/// This defines the interface for openraft network transport.
#[async_trait::async_trait]
pub trait RaftNetworkConnection: Send + Sync {
    /// Send append entries RPC to a peer
    async fn send_append_entries(
        &self,
        target: u64,
        rpc: openraft::raft::AppendEntriesRequest<crate::cluster::types::TypeConfig>,
    ) -> Result<openraft::raft::AppendEntriesResponse<u64>, Box<dyn std::error::Error + Send + Sync>>;

    /// Send vote request RPC to a peer
    async fn send_vote(
        &self,
        target: u64,
        rpc: openraft::raft::VoteRequest<u64>,
    ) -> Result<openraft::raft::VoteResponse<u64>, Box<dyn std::error::Error + Send + Sync>>;

    /// Send install snapshot RPC to a peer
    async fn send_install_snapshot(
        &self,
        target: u64,
        rpc: openraft::raft::InstallSnapshotRequest<crate::cluster::types::TypeConfig>,
    ) -> Result<openraft::raft::InstallSnapshotResponse<u64>, Box<dyn std::error::Error + Send + Sync>>;
}
