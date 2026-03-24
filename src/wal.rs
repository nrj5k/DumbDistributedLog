//! Write-Ahead Log (WAL) for DDL durability
//!
//! Ensures entries survive crashes by writing to disk before acknowledging.
//! Uses waly for append-only log files.

use waly::WriteAheadLog;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use crate::traits::ddl::Entry;
use log::{info, debug, warn};

/// WAL-backed storage for a single topic
pub struct TopicWal {
    /// The waly instance for this topic
    wal: Arc<Mutex<WriteAheadLog>>,
    /// Topic name
    topic: String,
    /// Path to WAL file
    path: PathBuf,
    /// Last entry ID (protected by mutex for thread-safe updates during recovery)
    last_id: Arc<Mutex<u64>>,
}

impl TopicWal {
    /// Create or open a WAL for a topic
    pub fn open(topic: &str, data_dir: &Path) -> Result<Self, WalError> {
        let sanitized = sanitize_topic(topic)?;
        let topic_dir = data_dir.join("wal").join(sanitized);
        std::fs::create_dir_all(&topic_dir)?;
        
        let path = topic_dir.join("entries.wal");
        
        // Create WAL with default options
        let wal = WriteAheadLog::new(&path)?;
        
        // Get last entry ID by reading existing entries
        let last_id = wal.read_all()
            .map(|entries| entries.iter().map(|e| e.id).max().unwrap_or(0))
            .unwrap_or(0);
        
        info!("Opened WAL for topic '{}' at {:?} with last_id={}", topic, path, last_id);
        
        Ok(Self {
            wal: Arc::new(Mutex::new(wal)),
            topic: topic.to_string(),
            path,
            last_id: Arc::new(Mutex::new(last_id)),
        })
    }
    
    /// Append an entry to the WAL
    pub async fn append(&self, entry: &Entry) -> Result<u64, WalError> {
        // Serialize entry using JSON (NOT bincode - it's archived!)
        let data = serde_json::to_vec(entry)?;
        
        // Write to WAL - the waly library handles durability
        let mut wal = self.wal.lock().await;
        let log_entry = wal.append(data)?;
        
        // Update our logical entry ID
        let mut last_id = self.last_id.lock().await;
        *last_id = log_entry.id;
        
        debug!("WAL appended entry {} to topic '{}'", entry.id, self.topic);
        
        Ok(log_entry.id)
    }
    
    /// Read entries from WAL (for recovery)
    pub async fn read_entries(&self) -> Result<Vec<Entry>, WalError> {
        let wal = self.wal.lock().await;
        
        // Read all entries from the waly WAL
        let log_entries = wal.read_all()?;
        
        let mut entries = Vec::new();
        
        for log_entry in log_entries {
            // Deserialize each entry from the stored data
            match serde_json::from_slice::<Entry>(&log_entry.data) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    // Log error but continue - don't fail entire recovery
                    warn!("Failed to deserialize WAL entry {}: {:?}", log_entry.id, e);
                    continue;
                }
            }
        }
        
        // Sort by ID to ensure order
        entries.sort_by_key(|e| e.id);
        
        // Update last_id based on recovered entries
        let mut last_id = self.last_id.lock().await;
        if let Some(last) = entries.last() {
            *last_id = last.id;
        }
        
        info!("Recovered {} entries from WAL for topic '{}'", entries.len(), self.topic);
        Ok(entries)
    }
    
    /// Sync WAL to disk (ensure durability)
    pub async fn sync(&self) -> Result<(), WalError> {
        // waly flushes on each append, so no explicit sync needed
        Ok(())
    }
    
    /// Get last written ID
    pub async fn last_id(&self) -> u64 {
        *self.last_id.lock().await
    }
}

/// Sanitize topic name for filesystem with security validation.
/// 
/// Returns an error for:
/// - Empty topic names
/// - Topic names exceeding 255 characters
/// - Topic names containing ".." (path traversal)
/// - Topic names containing control characters
fn sanitize_topic(topic: &str) -> Result<String, WalError> {
    // Check for empty topic name
    if topic.is_empty() {
        return Err(WalError::InvalidTopic("topic name cannot be empty".to_string()));
    }
    
    // Check for excessively long topic names
    if topic.len() > 255 {
        return Err(WalError::InvalidTopic(format!(
            "topic name too long: {} characters (max 255)",
            topic.len()
        )));
    }
    
    // Check for path traversal attempts
    if topic.contains("..") {
        return Err(WalError::InvalidTopic("topic name cannot contain '..'".to_string()));
    }
    
    // Check for control characters
    if topic.chars().any(|c| c.is_control()) {
        return Err(WalError::InvalidTopic(
            "topic name cannot contain control characters".to_string(),
        ));
    }
    
    // Sanitize by replacing unsafe filesystem characters
    Ok(topic.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"))
}

/// Un-sanitize topic name from filesystem
fn unsanitize_topic(sanitized: &str) -> String {
    // We can't perfectly reverse the sanitization since we replaced multiple chars with "_"
    // For now, we just return the sanitized name; this is primarily for display/debugging
    // The actual topic name is stored in the Entry anyway
    sanitized.to_string()
}

/// WAL manager for all topics
pub struct WalManager {
    /// Data directory
    data_dir: PathBuf,
    /// Open WALs per topic
    wals: Arc<RwLock<HashMap<String, TopicWal>>>,
}

impl WalManager {
    /// Create new WAL manager
    pub fn new(data_dir: &Path) -> Self {
        std::fs::create_dir_all(data_dir).expect("Failed to create data directory");
        
        Self {
            data_dir: data_dir.to_path_buf(),
            wals: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get or create WAL for a topic
    pub async fn get_or_create(&self, topic: &str) -> Result<TopicWal, WalError> {
        // Check if already open
        {
            let wals = self.wals.read().await;
            if let Some(wal) = wals.get(topic) {
                return Ok(wal.clone());
            }
        }
        
        // Create new
        let wal = TopicWal::open(topic, &self.data_dir)?;
        
        // Store
        let mut wals = self.wals.write().await;
        wals.insert(topic.to_string(), wal.clone());
        
        Ok(wal)
    }
    
    /// Append to a topic's WAL
    pub async fn append(&self, topic: &str, entry: &Entry) -> Result<u64, WalError> {
        // First ensure the WAL exists
        let wal = self.get_or_create(topic).await?;
        // Now append (we don't need write lock since TopicWal::append is thread-safe)
        wal.append(entry).await
    }
    
    /// Recover all topics from WAL
    pub async fn recover_all(&self) -> Result<HashMap<String, Vec<Entry>>, WalError> {
        let mut recovered = HashMap::new();
        
        // Scan data directory for all topic WALs
        let wal_dir = self.data_dir.join("wal");
        if !wal_dir.exists() {
            debug!("WAL directory does not exist, nothing to recover");
            return Ok(recovered);
        }
        
        let entries = match std::fs::read_dir(&wal_dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read WAL directory: {:?}", e);
                return Ok(recovered);
            }
        };
        
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {:?}", e);
                    continue;
                }
            };
            
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    warn!("Failed to get file type: {:?}", e);
                    continue;
                }
            };
            
            if !file_type.is_dir() {
                continue;
            }
            
            let sanitized_topic = entry.file_name().to_string_lossy().to_string();
            let topic = unsanitize_topic(&sanitized_topic);
            
            info!("Found WAL for topic: {}", topic);
            
            // Open WAL and read entries
            let wal = match self.get_or_create(&topic).await {
                Ok(w) => w,
                Err(e) => {
                    warn!("Failed to open WAL for topic '{}': {:?}", topic, e);
                    continue;
                }
            };
            
            let topic_entries = match wal.read_entries().await {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read entries for topic '{}': {:?}", topic, e);
                    continue;
                }
            };
            
            if !topic_entries.is_empty() {
                recovered.insert(topic, topic_entries);
            }
        }
        
        info!("Recovered {} topics from WAL", recovered.len());
        Ok(recovered)
    }
    
    /// Sync all WALs to disk
    pub async fn sync_all(&self) -> Result<(), WalError> {
        let wals = self.wals.read().await;
        for (topic, wal) in wals.iter() {
            if let Err(e) = wal.sync().await {
                warn!("Failed to sync WAL for topic '{}': {:?}", topic, e);
            }
        }
        Ok(())
    }
    
    /// List all topics with WALs
    pub async fn list_topics(&self) -> Vec<String> {
        let wals = self.wals.read().await;
        wals.keys().cloned().collect()
    }
}

impl Clone for TopicWal {
    fn clone(&self) -> Self {
        // Clone the Arc references (both the WAL and last_id are already Arc<Mutex<>>)
        Self {
            wal: Arc::clone(&self.wal),
            topic: self.topic.clone(),
            path: self.path.clone(),
            last_id: Arc::clone(&self.last_id),
        }
    }
}

/// WAL errors
#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("WAL error: {0}")]
    Wal(String),
    
    #[error("Topic not found: {0}")]
    TopicNotFound(String),
    
    #[error("Invalid topic name: {0}")]
    InvalidTopic(String),
}

impl From<waly::WalError> for WalError {
    fn from(e: waly::WalError) -> Self {
        WalError::Wal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_wal_sanity_check() {
        let tmp = TempDir::new().unwrap();
        // "/" is sanitized to "_", so this should work
        let wal = TopicWal::open("test/topic", tmp.path());
        assert!(wal.is_ok());
    }
    
    #[test]
    fn test_sanitize_topic_valid() {
        // Valid topic names
        assert!(sanitize_topic("valid_topic").is_ok());
        assert!(sanitize_topic("topic123").is_ok());
        assert!(sanitize_topic("my-topic").is_ok());
    }
    
    #[test]
    fn test_sanitize_topic_sanitizes_chars() {
        // Characters should be sanitized
        let result = sanitize_topic("test/topic").unwrap();
        assert_eq!(result, "test_topic");
        
        let result = sanitize_topic("test:topic").unwrap();
        assert_eq!(result, "test_topic");
        
        let result = sanitize_topic("test\\topic").unwrap();
        assert_eq!(result, "test_topic");
    }
    
    #[test]
    fn test_sanitize_topic_empty() {
        // Empty topic should fail
        assert!(sanitize_topic("").is_err());
    }
    
    #[test]
    fn test_sanitize_topic_path_traversal() {
        // Path traversal should fail
        assert!(sanitize_topic("../etc/passwd").is_err());
        assert!(sanitize_topic("topic/../etc").is_err());
        assert!(sanitize_topic("..").is_err());
    }
    
    #[test]
    fn test_sanitize_topic_too_long() {
        // Long topic names should fail
        let long_topic = "a".repeat(300);
        assert!(sanitize_topic(&long_topic).is_err());
    }
    
    #[test]
    fn test_sanitize_topic_control_chars() {
        // Control characters should fail
        assert!(sanitize_topic("topic\x00name").is_err());
        assert!(sanitize_topic("topic\x1fname").is_err());
        assert!(sanitize_topic("topic\nname").is_err());
    }
    
    #[tokio::test]
    async fn test_wal_recovery() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create WAL and write entry
        {
            let wal = TopicWal::open("test", temp_dir.path()).unwrap();
            let entry = Entry { 
                id: 1, 
                timestamp: 123, 
                topic: "test".into(),  // Convert &str to Arc<str>
                payload: vec![1, 2, 3].into()  // Convert Vec<u8> to Arc<[u8]>
            };
            wal.append(&entry).await.unwrap();
        }
        
        // Re-open and recover - waly shares the same file handle
        let wal = TopicWal::open("test", temp_dir.path()).unwrap();
        let entries = wal.read_entries().await.unwrap();
        
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
        assert_eq!(&*entries[0].payload, &[1, 2, 3]);
    }
    
    #[tokio::test]
    async fn test_wal_multiple_entries_recovery() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create WAL and write multiple entries
        {
            let wal = TopicWal::open("multi-test", temp_dir.path()).unwrap();
            for i in 1..=5 {
                let entry = Entry { 
                    id: i, 
                    timestamp: i * 100, 
                    topic: "multi-test".into(),  // Convert &str to Arc<str>
                    payload: vec![i as u8; 10].into()  // Convert Vec<u8> to Arc<[u8]>
                };
                wal.append(&entry).await.unwrap();
            }
        }
        
        // Re-open and recover
        let wal = TopicWal::open("multi-test", temp_dir.path()).unwrap();
        let entries = wal.read_entries().await.unwrap();
        
        assert_eq!(entries.len(), 5);
        // Verify entries are sorted by ID
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.id, (i + 1) as u64);
        }
    }
    
    #[tokio::test]
    async fn test_wal_manager_recover_all() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WalManager::new(temp_dir.path());
        
        // Write entries to multiple topics
        {
            let wal1 = manager.get_or_create("topic1").await.unwrap();
            let entry1 = Entry { 
                id: 1, 
                timestamp: 1, 
                topic: "topic1".into(),  // Convert &str to Arc<str>
                payload: vec![1].into()  // Convert Vec<u8> to Arc<[u8]>
            };
            wal1.append(&entry1).await.unwrap();
            
            let wal2 = manager.get_or_create("topic2").await.unwrap();
            let entry2 = Entry { 
                id: 1, 
                timestamp: 2, 
                topic: "topic2".into(),  // Convert &str to Arc<str>
                payload: vec![2].into()  // Convert Vec<u8> to Arc<[u8]>
            };
            wal2.append(&entry2).await.unwrap();
        }
        
        // Recover all
        let recovered = manager.recover_all().await.unwrap();
        
        // Should have both topics
        assert_eq!(recovered.len(), 2);
        assert!(recovered.contains_key("topic1"));
        assert!(recovered.contains_key("topic2"));
        
        // Verify entries
        let topic1_entries = recovered.get("topic1").unwrap();
        assert_eq!(topic1_entries.len(), 1);
        assert_eq!(&*topic1_entries[0].payload, &[1]);
    }
}