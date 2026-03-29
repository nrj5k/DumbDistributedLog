//! Raft storage implementation for AutoQueues
//!
//! Provides in-memory storage for Raft log entries and state machine.
//! Uses RaftStorage trait from openraft for compatibility.
//! Supports synchronous file persistence for durability across restarts.

use openraft::storage::{RaftLogReader, RaftSnapshotBuilder, RaftStorage};
use openraft::{
    Entry, ErrorSubject, ErrorVerb, LogId, LogState, RaftTypeConfig, Snapshot, SnapshotMeta,
    StoredMembership, StorageError, Vote,
};
use openraft::impls::BasicNode;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex;

use crate::cluster::ownership_machine::{OwnershipCommand, OwnershipResponse, OwnershipState};
use crate::cluster::types::{SerializableLogEntry, SerializableVote, TypeConfig};

/// In-memory storage implementation for AutoQueues Raft cluster
pub struct AutoqueuesRaftStorage {
    /// Raft log entries
    log: RwLock<BTreeMap<u64, Entry<TypeConfig>>>,
    /// Path to persisted log entries
    log_file: Option<PathBuf>,
    /// Current vote state
    vote: RwLock<Option<Vote<u64>>>,
    /// Path to persisted vote
    vote_file: Option<PathBuf>,
    /// Last applied log ID
    last_applied: RwLock<Option<LogId<u64>>>,
    /// Committed log ID
    committed: RwLock<Option<Option<LogId<u64>>>>,
    /// Snapshot metadata
    snapshot_meta: RwLock<Option<SnapshotMeta<u64, BasicNode>>>,
    /// Snapshot data
    snapshot: RwLock<Option<Snapshot<TypeConfig>>>,
    /// Shared ownership state - Arc for sharing across clones
    pub ownership_state: Arc<RwLock<OwnershipState>>,
    /// Current membership
    membership: RwLock<StoredMembership<u64, BasicNode>>,
    /// Path to persistence file (optional)
    data_file: Option<PathBuf>,
    /// Dirty flag shared across clones for ownership state
    dirty: Arc<AtomicBool>,
    /// Lock to prevent concurrent flushes
    flush_lock: Arc<Mutex<()>>,
}

impl AutoqueuesRaftStorage {
    /// Path to log file (constant)
    const LOG_FILE_NAME: &'static str = "raft_log.dat";

    /// Create a new in-memory Raft storage instance
    pub fn new() -> Self {
        Self {
            log: RwLock::new(BTreeMap::new()),
            log_file: None,
            vote: RwLock::new(None),
            vote_file: None,
            last_applied: RwLock::new(None),
            committed: RwLock::new(None),
            snapshot_meta: RwLock::new(None),
            snapshot: RwLock::new(None),
            ownership_state: Arc::new(RwLock::new(OwnershipState::new())),
            membership: RwLock::new(StoredMembership::default()),
            data_file: None,
            dirty: Arc::new(AtomicBool::new(false)),
            flush_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Create a new Raft storage with file persistence
    pub fn with_persistence(data_dir: impl AsRef<std::path::Path>) -> Result<Self, StorageError<u64>> {
        std::fs::create_dir_all(data_dir.as_ref())
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Store, ErrorVerb::Write, e))?;

        let data_file = data_dir.as_ref().join("ownership_state.bin");
        let log_file = data_dir.as_ref().join(Self::LOG_FILE_NAME);
        let vote_file = data_dir.as_ref().join("vote.dat");

        let storage = Self {
            log: RwLock::new(BTreeMap::new()),
            log_file: Some(log_file.clone()),
            vote: RwLock::new(None),
            vote_file: Some(vote_file.clone()),
            last_applied: RwLock::new(None),
            committed: RwLock::new(None),
            snapshot_meta: RwLock::new(None),
            snapshot: RwLock::new(None),
            ownership_state: Arc::new(RwLock::new(OwnershipState::new())),
            membership: RwLock::new(StoredMembership::default()),
            data_file: Some(data_file.clone()),
            dirty: Arc::new(AtomicBool::new(false)),
            flush_lock: Arc::new(Mutex::new(())),
        };

        // Load existing state if files exist
        if data_file.exists() {
            storage.load_from_file(&data_file)?;
        }
        if log_file.exists() {
            storage.load_log_from_file(&log_file)?;
        }
        if vote_file.exists() {
            storage.load_vote_from_file(&vote_file)?;
        }

        Ok(storage)
    }

    /// Load ownership state from file
    fn load_from_file(&self, path: &PathBuf) -> Result<(), StorageError<u64>> {
        use std::fs::File;
        use std::io::Read;

        let file = File::open(path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Snapshot(None), ErrorVerb::Read, e))?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Snapshot(None), ErrorVerb::Read, e))?;

        if !buffer.is_empty() {
            let state: OwnershipState = bincode::deserialize(&buffer)
                .map_err(|e| {
                    let io_err = std::io::Error::new(std::io::ErrorKind::InvalidData, e);
                    StorageError::from_io_error(ErrorSubject::Snapshot(None), ErrorVerb::Read, io_err)
                })?;
            *self.ownership_state.write().unwrap() = state;
        }

        Ok(())
    }

    /// Flush ownership state to file (synchronous, thread-safe)
    fn flush_to_file(&self) -> Result<(), StorageError<u64>> {
        // Try to acquire flush lock (non-blocking)
        let _guard = self.flush_lock.try_lock()
            .map_err(|_| StorageError::from_io_error(
                ErrorSubject::Logs,
                ErrorVerb::Write,
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "Flush already in progress"),
            ))?;

        let path = self.data_file.as_ref().ok_or_else(|| {
            let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "No data file configured");
            StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, io_err)
        })?;

        let state = self.ownership_state.read().unwrap();

        // Atomic write: write to temp, then rename
        let temp_path = path.with_extension("tmp");
        {
            let file = std::fs::File::create(&temp_path)
                .map_err(|e| StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, e))?;
            let writer = std::io::BufWriter::new(file);
            bincode::serialize_into(writer, &*state)
                .map_err(|e| StorageError::from_io_error(
                    ErrorSubject::Logs,
                    ErrorVerb::Write,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                ))?;
        }

        // Atomic rename (POSIX guarantees atomicity)
        std::fs::rename(&temp_path, path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, e))?;

        Ok(())
    }

    /// Load log entries from file
    fn load_log_from_file(&self, path: &PathBuf) -> Result<(), StorageError<u64>> {
        let file = std::fs::File::open(path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Read, e))?;
        let reader = std::io::BufReader::new(file);
        let entries: Vec<SerializableLogEntry> = bincode::deserialize_from(reader)
            .map_err(|e| StorageError::from_io_error(
                ErrorSubject::Logs,
                ErrorVerb::Read,
                std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            ))?;

        let mut log = self.log.write().unwrap();
        for se in entries {
            let entry: Entry<TypeConfig> = se.into();
            log.insert(entry.log_id.index, entry);
        }

        Ok(())
    }

    /// Persist log entries to file
    fn flush_log_to_file(&self) -> Result<(), StorageError<u64>> {
        let path = self.log_file.as_ref().ok_or_else(|| {
            let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "No log file configured");
            StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, io_err)
        })?;

        let log = self.log.read().unwrap();
        let entries: Vec<SerializableLogEntry> = log.values()
            .map(|e| SerializableLogEntry::from(e))
            .collect();
        drop(log);

        // Atomic write: write to temp, then rename
        let temp_path = path.with_extension("tmp");
        {
            let file = std::fs::File::create(&temp_path)
                .map_err(|e| StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, e))?;
            let writer = std::io::BufWriter::new(file);
            bincode::serialize_into(writer, &entries)
                .map_err(|e| StorageError::from_io_error(
                    ErrorSubject::Logs,
                    ErrorVerb::Write,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                ))?;
        }

        // Atomic rename (POSIX guarantees atomicity)
        std::fs::rename(&temp_path, path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Logs, ErrorVerb::Write, e))?;

        Ok(())
    }

    /// Load vote from file
    fn load_vote_from_file(&self, path: &PathBuf) -> Result<(), StorageError<u64>> {
        let file = std::fs::File::open(path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Vote, ErrorVerb::Read, e))?;
        let reader = std::io::BufReader::new(file);
        let vote_data: SerializableVote = bincode::deserialize_from(reader)
            .map_err(|e| StorageError::from_io_error(
                ErrorSubject::Vote,
                ErrorVerb::Read,
                std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            ))?;

        let mut vote = Vote::new(vote_data.term, vote_data.node_id);
        if vote_data.committed {
            vote.commit();
        }
        *self.vote.write().unwrap() = Some(vote);

        Ok(())
    }

    /// Persist vote to file
    fn flush_vote_to_file(&self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        let path = self.vote_file.as_ref().ok_or_else(|| {
            let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "No vote file configured");
            StorageError::from_io_error(ErrorSubject::Vote, ErrorVerb::Write, io_err)
        })?;

        let vote_data = SerializableVote {
            term: vote.leader_id.term,
            node_id: vote.leader_id.node_id,
            committed: vote.committed,
        };

        // Atomic write: write to temp, then rename
        let temp_path = path.with_extension("tmp");
        {
            let file = std::fs::File::create(&temp_path)
                .map_err(|e| StorageError::from_io_error(ErrorSubject::Vote, ErrorVerb::Write, e))?;
            bincode::serialize_into(std::io::BufWriter::new(file), &vote_data)
                .map_err(|e| StorageError::from_io_error(
                    ErrorSubject::Vote,
                    ErrorVerb::Write,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                ))?;
        }

        // Atomic rename (POSIX guarantees atomicity)
        std::fs::rename(&temp_path, path)
            .map_err(|e| StorageError::from_io_error(ErrorSubject::Vote, ErrorVerb::Write, e))?;

        Ok(())
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
            log_file: self.log_file.clone(),
            vote: RwLock::new(*self.vote.read().unwrap()),
            vote_file: self.vote_file.clone(),
            last_applied: RwLock::new(*self.last_applied.read().unwrap()),
            committed: RwLock::new(*self.committed.read().unwrap()),
            snapshot_meta: RwLock::new(self.snapshot_meta.read().unwrap().clone()),
            snapshot: RwLock::new(self.snapshot.read().unwrap().clone()),
            // Share the same ownership state - this is the fix for thread safety
            ownership_state: Arc::clone(&self.ownership_state),
            membership: RwLock::new(self.membership.read().unwrap().clone()),
            data_file: self.data_file.clone(),
            dirty: Arc::clone(&self.dirty),  // Share dirty flag
            flush_lock: Arc::clone(&self.flush_lock),  // Share flush lock
        }
    }
}

impl RaftStorage<TypeConfig> for AutoqueuesRaftStorage {
    type SnapshotBuilder = Self;
    type LogReader = Self;

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        *self.vote.write().unwrap() = Some(*vote);

        // Persist vote
        if self.log_file.is_some() {
            self.flush_vote_to_file(vote)?;
        }

        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        Ok(*self.vote.read().unwrap())
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<u64>>,
    ) -> Result<(), StorageError<u64>> {
        *self.committed.write().unwrap() = Some(committed);
        Ok(())
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        Ok(self.committed.read().unwrap().flatten())
    }

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<u64>> {
        let log = self.log.read().unwrap();

        let last_purged_log_id = *self.last_applied.read().unwrap();

        let last_log_id = if log.is_empty() {
            last_purged_log_id
        } else {
            let last_idx = *log.keys().next_back().unwrap();
            let entry = log.get(&last_idx).unwrap();
            Some(entry.log_id.clone())
        };

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<u64>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
    {
        // Add to in-memory log
        {
            let mut log = self.log.write().unwrap();
            for entry in entries {
                log.insert(entry.log_id.index, entry);
            }
        }

        // Persist to disk
        if self.log_file.is_some() {
            self.flush_log_to_file()?;
        }

        Ok(())
    }

    async fn delete_conflict_logs_since(
        &mut self,
        log_id: LogId<u64>,
    ) -> Result<(), StorageError<u64>> {
        let mut log = self.log.write().unwrap();
        let keys: Vec<u64> = log.keys().filter(|&&k| k >= log_id.index).cloned().collect();
        for key in keys {
            log.remove(&key);
        }

        // Persist changes
        if self.log_file.is_some() {
            drop(log); // Release lock before flush
            self.flush_log_to_file()?;
        }

        Ok(())
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let mut log = self.log.write().unwrap();
        let keys: Vec<u64> = log.keys().filter(|&&k| k <= log_id.index).cloned().collect();
        for key in keys {
            log.remove(&key);
        }

        // Persist changes
        if self.log_file.is_some() {
            drop(log); // Release lock before flush
            self.flush_log_to_file()?;
        }

        Ok(())
    }

    async fn last_applied_state(
        &mut self,
    ) -> Result<(Option<LogId<u64>>, StoredMembership<u64, BasicNode>), StorageError<u64>> {
        let last_applied = *self.last_applied.read().unwrap();
        let membership = self.membership.read().unwrap().clone();
        Ok((last_applied, membership))
    }

    async fn apply_to_state_machine(
        &mut self,
        entries: &[Entry<TypeConfig>],
    ) -> Result<Vec<OwnershipResponse>, StorageError<u64>> {
        let mut result = Vec::new();
        let mut state = self.ownership_state.write().unwrap();

        for entry in entries {
            self.last_applied.write().unwrap().replace(entry.log_id);

            if let openraft::entry::EntryPayload::Normal(ref cmd) = entry.payload {
                let topic = match cmd {
                    OwnershipCommand::ClaimTopic { topic, .. } => topic.clone(),
                    OwnershipCommand::ReleaseTopic { topic, .. } => topic.clone(),
                    OwnershipCommand::TransferTopic { topic, .. } => topic.clone(),
                };

                state.apply(cmd);
                self.dirty.store(true, Ordering::Release);

                let owner = state.get_owner(&topic);
                result.push(OwnershipResponse::Owner { topic, owner });
            }
        }

        drop(state);

        // Synchronous flush - blocks until complete
        if self.data_file.is_some() && self.dirty.load(Ordering::Acquire) {
            self.flush_to_file()?;
            self.dirty.store(false, Ordering::Release);
        }

        Ok(result)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<<TypeConfig as RaftTypeConfig>::SnapshotData>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, <TypeConfig as RaftTypeConfig>::Node>,
        snapshot: Box<<TypeConfig as RaftTypeConfig>::SnapshotData>,
    ) -> Result<(), StorageError<u64>> {
        // Store metadata
        *self.snapshot_meta.write().unwrap() = Some(meta.clone());

        // Deserialize ownership state from snapshot
        let mut cursor = snapshot;
        let mut data = Vec::new();
        use std::io::Read;
        cursor.read_to_end(&mut data)
            .map_err(|e| StorageError::from_io_error(
                ErrorSubject::Snapshot(None),
                ErrorVerb::Read,
                e,
            ))?;

        if !data.is_empty() {
            let state: OwnershipState = bincode::deserialize(&data)
                .map_err(|e| StorageError::from_io_error(
                    ErrorSubject::Snapshot(None),
                    ErrorVerb::Read,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                ))?;

            // Restore ownership state
            *self.ownership_state.write().unwrap() = state;

            // Mark dirty and flush to disk
            self.dirty.store(true, Ordering::Release);
            if self.data_file.is_some() {
                self.flush_to_file()?;
                self.dirty.store(false, Ordering::Release);
            }
        }

        // Store snapshot
        *self.snapshot.write().unwrap() = Some(Snapshot {
            meta: meta.clone(),
            snapshot: Box::new(Cursor::new(data)),
        });

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<u64>> {
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
            std::ops::Bound::Unbounded => {
                if log.is_empty() {
                    0
                } else {
                    *log.keys().next_back().unwrap() + 1
                }
            }
        };
        Ok(log.range(start..end).map(|(_, v): (&u64, &Entry<TypeConfig>)| v.clone()).collect())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for AutoqueuesRaftStorage {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<u64>> {
        let meta = self.snapshot_meta.read().unwrap().clone().unwrap_or_default();

        // Serialize ownership state
        let state = self.ownership_state.read().unwrap();
        let snapshot_data = bincode::serialize(&*state)
            .map_err(|e| StorageError::from_io_error(
                ErrorSubject::Snapshot(None),
                ErrorVerb::Write,
                std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            ))?;

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(snapshot_data)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_storage_creation() {
        let storage = AutoqueuesRaftStorage::new();
        assert!(storage.ownership_state.read().unwrap().topic_count() == 0);
    }

    #[test]
    fn test_storage_clone() {
        let storage = AutoqueuesRaftStorage::new();
        let cloned = storage.clone();
        assert!(cloned.ownership_state.read().unwrap().topic_count() == 0);
    }

    #[test]
    fn test_storage_clone_shares_state() {
        // Verify that clones share the same ownership state (Issue 1 fix)
        let storage = AutoqueuesRaftStorage::new();

        // State machine update on original
        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "test.topic".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        // Clone should see the same state
        let cloned = storage.clone();
        let owner = cloned.ownership_state.read().unwrap().get_owner("test.topic");
        assert_eq!(owner, Some(1), "Clone should share ownership state");
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        // Create storage with persistence
        let storage = AutoqueuesRaftStorage::with_persistence(path).expect("Failed to create storage with persistence");

        // Apply some state changes
        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.cpu".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        storage.dirty.store(true, Ordering::Release);

        // Force flush
        storage.flush_to_file().expect("Failed to flush");

        // Create new storage to load persisted state
        let storage2 = AutoqueuesRaftStorage::with_persistence(path).expect("Failed to create second storage");

        // Should have persisted state
        let owner = storage2.ownership_state.read().unwrap().get_owner("metrics.cpu");
        assert_eq!(owner, Some(1), "Persisted state should be loaded");
    }

    #[test]
    fn test_ownership_state_persistence() {
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        // Create storage and apply commands
        let storage = AutoqueuesRaftStorage::with_persistence(path).expect("Failed to create storage");

        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "topic1".to_string(),
            node_id: 1,
            timestamp: 1000,
        });

        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "topic2".to_string(),
            node_id: 2,
            timestamp: 1001,
        });

        storage.dirty.store(true, Ordering::Release);
        storage.flush_to_file().expect("Failed to flush");

        // Create new storage and verify state loaded
        let storage2 = AutoqueuesRaftStorage::with_persistence(path).expect("Failed to create second storage");
        assert_eq!(storage2.ownership_state.read().unwrap().get_owner("topic1"), Some(1));
        assert_eq!(storage2.ownership_state.read().unwrap().get_owner("topic2"), Some(2));
    }

    #[test]
    fn test_atomic_write() {
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        let storage = AutoqueuesRaftStorage::with_persistence(path).expect("Failed to create storage");

        // Write data
        storage.ownership_state.write().unwrap().apply(&OwnershipCommand::ClaimTopic {
            topic: "test".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
        storage.flush_to_file().expect("Failed to flush");

        // Atomic rename should have created the file
        assert!(path.join("ownership_state.bin").exists(), "Ownership state file should exist");

        // Temp file should be cleaned up
        assert!(!path.join("ownership_state.tmp").exists(), "Temp file should be cleaned up");
    }
}