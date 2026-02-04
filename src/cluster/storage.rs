use openraft::storage::{RaftLogReader, RaftSnapshotBuilder, RaftStorage};
use openraft::{LogId, Snapshot, SnapshotMeta, StorageError, Vote, Entry, RaftTypeConfig, LogState, StoredMembership, CommittedLeaderId};
use openraft::impls::BasicNode;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::RwLock;
use std::ops::RangeBounds;

use crate::cluster::types::{EntryData, TypeConfig};

/// In-memory storage implementation for AutoQueues Raft cluster
pub struct AutoqueuesRaftStorage {
    /// Raft log entries
    log: RwLock<BTreeMap<u64, Entry<TypeConfig>>>,
    /// Current vote state
    vote: RwLock<Option<Vote<u64>>>,
    /// Last applied log ID
    last_applied: RwLock<Option<LogId<u64>>>,
    /// Committed log ID
    committed: RwLock<Option<LogId<u64>>>,
    /// Snapshot metadata
    snapshot_meta: RwLock<Option<SnapshotMeta<u64, BasicNode>>>,
    /// Snapshot data
    snapshot: RwLock<Option<Snapshot<TypeConfig>>>,
}

impl AutoqueuesRaftStorage {
    /// Create a new in-memory Raft storage instance
    pub fn new() -> Self {
        Self {
            log: RwLock::new(BTreeMap::new()),
            vote: RwLock::new(None),
            last_applied: RwLock::new(None),
            committed: RwLock::new(None),
            snapshot_meta: RwLock::new(None),
            snapshot: RwLock::new(None),
        }
    }
    
    /// Create a new in-memory Raft storage instance with default values
    pub fn default() -> Self {
        Self::new()
    }
}

impl Default for AutoqueuesRaftStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AutoqueuesRaftStorage {
    fn clone(&self) -> Self {
        Self {
            log: RwLock::new(self.log.read().unwrap().clone()),
            vote: RwLock::new(*self.vote.read().unwrap()),
            last_applied: RwLock::new(*self.last_applied.read().unwrap()),
            committed: RwLock::new(*self.committed.read().unwrap()),
            snapshot_meta: RwLock::new(self.snapshot_meta.read().unwrap().clone()),
            snapshot: RwLock::new(self.snapshot.read().unwrap().clone()),
        }
    }
}

impl RaftStorage<TypeConfig> for AutoqueuesRaftStorage {
    type SnapshotBuilder = Self;
    type LogReader = Self;

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        *self.vote.write().unwrap() = Some(*vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        Ok(*self.vote.read().unwrap())
    }

    async fn save_committed(&mut self, committed: Option<LogId<u64>>) -> Result<(), StorageError<u64>> {
        *self.committed.write().unwrap() = committed;
        Ok(())
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        Ok(*self.committed.read().unwrap())
    }

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<u64>> {
        let log = self.log.read().unwrap();
        if log.is_empty() {
            return Ok(LogState::default());
        }
        
        let last = *log.keys().next_back().unwrap();
        
        // In a real implementation, you'd track the purged log ID separately
        let last_purged_log_id = None;
        
        Ok(LogState {
            last_purged_log_id,
            last_log_id: Some(LogId::new(CommittedLeaderId::new(1, 1), last)), // Using term 1 for simplicity
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<u64>>
    where I: IntoIterator<Item = Entry<TypeConfig>> + Send {
        let mut log = self.log.write().unwrap();
        for entry in entries {
            log.insert(entry.log_id.index, entry);
        }
        Ok(())
    }

    async fn delete_conflict_logs_since(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let mut log = self.log.write().unwrap();
        let keys: Vec<u64> = log.keys().filter(|&&k| k >= log_id.index).cloned().collect();
        for key in keys {
            log.remove(&key);
        }
        Ok(())
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let mut log = self.log.write().unwrap();
        let keys: Vec<u64> = log.keys().filter(|&&k| k <= log_id.index).cloned().collect();
        for key in keys {
            log.remove(&key);
        }
        Ok(())
    }

    async fn last_applied_state(
        &mut self,
    ) -> Result<(Option<LogId<u64>>, StoredMembership<u64, BasicNode>), StorageError<u64>> {
        let last_applied = *self.last_applied.read().unwrap();
        // Return default membership for simplicity
        let membership = StoredMembership::default();
        Ok((last_applied, membership))
    }

    async fn apply_to_state_machine(&mut self, entries: &[Entry<TypeConfig>]) -> Result<Vec<EntryData>, StorageError<u64>> {
        // For now, just return the entry data from Normal entries
        let mut result = Vec::new();
        for entry in entries {
            if let openraft::entry::EntryPayload::Normal(ref data) = entry.payload {
                result.push(data.clone());
            }
        }
        Ok(result)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<<TypeConfig as RaftTypeConfig>::SnapshotData>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, <TypeConfig as RaftTypeConfig>::Node>,
        snapshot: Box<<TypeConfig as RaftTypeConfig>::SnapshotData>,
    ) -> Result<(), StorageError<u64>> {
        *self.snapshot_meta.write().unwrap() = Some(meta.clone());
        *self.snapshot.write().unwrap() = Some(Snapshot {
            meta: meta.clone(),
            snapshot,
        });
        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, StorageError<u64>> {
        Ok(self.snapshot.read().unwrap().clone())
    }
}

impl RaftLogReader<TypeConfig> for AutoqueuesRaftStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<u64>> {
        let log = self.log.read().unwrap();
        let start = match range.start_bound() {
            std::ops::Bound::Included(&i) => i,
            std::ops::Bound::Excluded(&i) => i + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(&i) => i + 1,
            std::ops::Bound::Excluded(&i) => i,
            std::ops::Bound::Unbounded => u64::MAX,
        };
        Ok(log.range(start..end).map(|(_, v)| v.clone()).collect())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for AutoqueuesRaftStorage {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<u64>> {
        let meta = self.snapshot_meta.read().unwrap().clone().unwrap_or_default();
        let data = Cursor::new(Vec::new());
        Ok(Snapshot {
            meta,
            snapshot: Box::new(data),
        })
    }
}