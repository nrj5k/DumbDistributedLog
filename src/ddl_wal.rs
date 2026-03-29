//! DDL implementation with Write-Ahead Log durability

use crate::ddl_distributed::DdlDistributed;
use crate::wal::{WalManager, WalError};
use crate::traits::ddl::{DDL, DdlConfig, DdlError, Entry, EntryStream};
use async_trait::async_trait;
use std::path::Path;
use tokio::sync::Mutex;
use log::warn;

impl From<WalError> for DdlError {
    fn from(error: WalError) -> Self {
        DdlError::Wal(error.to_string())
    }
}

/// DDL with WAL durability
pub struct DdlWithWal {
    /// Inner DDL implementation
    inner: DdlDistributed,
    /// WAL manager
    wal: WalManager,
    /// Sync interval (every N entries)
    sync_interval: usize,
    /// Entries since last sync
    entries_since_sync: std::sync::atomic::AtomicUsize,
    /// Mutex to serialize sync operations (tokio Mutex for async)
    sync_lock: Mutex<()>,
}

impl DdlWithWal {
    /// Create new DDL with WAL
    pub fn new(config: DdlConfig, data_dir: &Path) -> Result<Self, DdlError> {
        let inner = DdlDistributed::new_standalone(config);
        let wal = WalManager::new(data_dir);

        Ok(Self {
            inner,
            wal,
            sync_interval: 100,
            entries_since_sync: std::sync::atomic::AtomicUsize::new(0),
            sync_lock: Mutex::new(()),
        })
    }
}

#[async_trait]
impl DDL for DdlWithWal {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Push to in-memory first
        let id = self.inner.push(topic, payload.clone()).await?;
        
        // Create entry for WAL
        let entry = Entry::new(id, topic, payload);
        
        // Append to WAL - propagate errors since data won't be persisted on crash
        self.wal.append(topic, &entry).await?;
        
        // Periodic sync with proper synchronization
        // Use Mutex to prevent race condition between check-sync-reset
        let count = self.entries_since_sync.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count >= self.sync_interval {
            // Try to acquire sync lock - only one thread syncs at a time
            // Use try_lock to avoid blocking if another thread is syncing
            if let Ok(_guard) = self.sync_lock.try_lock() {
                if let Err(e) = self.wal.sync_all().await {
                    // Log sync errors but continue since data is already in WAL
                    warn!("Failed to sync WAL: {}", e);
                }
                // Reset counter inside the lock
                self.entries_since_sync.store(0, std::sync::atomic::Ordering::Release);
            }
            // If we couldn't acquire lock, another thread is syncing - that's fine
        }
        
        Ok(id)
    }
    
    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError> {
        self.inner.subscribe(topic).await
    }
    
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError> {
        self.inner.ack(topic, entry_id).await
    }
    
    async fn position(&self, topic: &str) -> Result<u64, DdlError> {
        self.inner.position(topic).await
    }
    
    fn owns_topic(&self, topic: &str) -> bool {
        self.inner.owns_topic(topic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_ddl_with_wal() {
        let tmp = TempDir::new().unwrap();
        let config = DdlConfig::default();
        let ddl = DdlWithWal::new(config, tmp.path()).unwrap();
        
        // Push data
        let id = ddl.push("test", vec![1, 2, 3]).await.unwrap();
        assert_eq!(id, 0);
    }
}