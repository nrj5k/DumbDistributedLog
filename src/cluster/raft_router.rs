//! Routes incoming Raft RPCs to the local Raft instance
//!
//! Bridges TCP server → Raft processing for multi-node coordination.

use crate::cluster::types::TypeConfig;
use openraft::error::{InstallSnapshotError, RaftError};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::Raft;
use std::sync::Arc;

/// Routes incoming RPCs to the Raft instance
pub struct RaftMessageRouter {
    raft: Arc<Raft<TypeConfig>>,
}

impl RaftMessageRouter {
    pub fn new(raft: Arc<Raft<TypeConfig>>) -> Self {
        Self { raft }
    }

    /// Handle incoming AppendEntries RPC
    pub async fn handle_append_entries(
        &self,
        rpc: AppendEntriesRequest<TypeConfig>,
    ) -> Result<AppendEntriesResponse<u64>, RaftError<u64>> {
        self.raft.append_entries(rpc).await
    }

    /// Handle incoming Vote RPC
    pub async fn handle_vote(
        &self,
        rpc: VoteRequest<u64>,
    ) -> Result<VoteResponse<u64>, RaftError<u64>> {
        self.raft.vote(rpc).await
    }

    /// Handle incoming InstallSnapshot RPC
    pub async fn handle_install_snapshot(
        &self,
        rpc: InstallSnapshotRequest<TypeConfig>,
    ) -> Result<InstallSnapshotResponse<u64>, RaftError<u64, InstallSnapshotError>> {
        self.raft.install_snapshot(rpc).await
    }
}
