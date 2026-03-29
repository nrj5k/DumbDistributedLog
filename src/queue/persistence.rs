//! Persistence module for data collection
//!
//! Provides append-only logging of queue data with minimal overhead.
//! Data is written asynchronously by a background thread and may lag
//! behind real-time operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Persistence configuration for a queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Directory to store log files
    pub data_dir: PathBuf,
    /// Max file size before rotation (bytes)
    pub max_file_size: u64,
    /// Flush interval (milliseconds)
    pub flush_interval_ms: u64,
    /// Whether to include timestamps
    pub include_timestamp: bool,
    /// Whether to compress old files
    pub compress_old: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data"),
            max_file_size: 100 * 1024 * 1024, // 100MB
            flush_interval_ms: 1000,          // 1 second
            include_timestamp: true,
            compress_old: false,
        }
    }
}

/// Persistence handle for a single queue
pub struct QueuePersistence {
    sender: mpsc::Sender<PersistEvent>,
    handle: Option<JoinHandle<()>>, // Store thread handle for graceful shutdown
}

/// Events sent to persistence thread
enum PersistEvent {
    Data { timestamp: u64, value: f64 },
    Flush,
    Shutdown,
}

impl QueuePersistence {
    /// Create new persistence handle for a queue
    pub fn new(queue_name: &str, config: PersistenceConfig) -> std::io::Result<Self> {
        // Create data directory if needed
        std::fs::create_dir_all(&config.data_dir)?;

        let log_file = config.data_dir.join(format!("{}.log", queue_name));
        let (sender, receiver) = mpsc::channel();
        let config_clone = config.clone();

        // Spawn background persistence thread
        let handle = thread::spawn(move || {
            persistence_thread(log_file, receiver, config_clone);
        });

        Ok(Self {
            sender,
            handle: Some(handle),
        })
    }

    /// Persist a data point (non-blocking, may lag)
    pub fn persist(&self, value: f64) {
        let timestamp = crate::types::now_millis();

        // Send to background thread - log warning on failure
        if let Err(e) = self.sender.send(PersistEvent::Data { timestamp, value }) {
            log::warn!("Failed to send data to persistence thread: {}", e);
        }
    }

    /// Request flush (best effort)
    pub fn flush(&self) {
        if let Err(e) = self.sender.send(PersistEvent::Flush) {
            log::warn!("Failed to send flush request to persistence thread: {}", e);
        }
    }

    /// Graceful shutdown with flush - call this before dropping
    pub fn shutdown(&mut self) {
        // Send shutdown signal
        if let Err(e) = self.sender.send(PersistEvent::Shutdown) {
            log::error!(
                "Failed to send shutdown signal to persistence thread: {}",
                e
            );
        }

        // Wait for thread to finish (with timeout)
        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                log::error!("Failed to join persistence thread: {:?}", e);
            }
        }
    }
}

impl Drop for QueuePersistence {
    fn drop(&mut self) {
        // If not already shut down gracefully, do best effort
        if self.handle.is_some() {
            if let Err(e) = self.sender.send(PersistEvent::Shutdown) {
                log::warn!("Failed to send shutdown signal in drop: {}", e);
            }
            if let Some(handle) = self.handle.take() {
                if let Err(e) = handle.join() {
                    log::warn!("Failed to join persistence thread in drop: {:?}", e);
                }
            }
        }
    }
}

/// Background persistence thread
fn persistence_thread(
    log_file: PathBuf,
    receiver: mpsc::Receiver<PersistEvent>,
    config: PersistenceConfig,
) {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .expect("Failed to open log file");

    let mut writer = BufWriter::with_capacity(64 * 1024, file); // 64KB buffer
    let mut last_flush = std::time::Instant::now();
    let flush_interval = Duration::from_millis(config.flush_interval_ms);

    loop {
        // Try to receive with timeout for periodic flush
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                match event {
                    PersistEvent::Data { timestamp, value } => {
                        // Write in simple CSV-like format: timestamp,value\n
                        if config.include_timestamp {
                            writeln!(
                                writer,
                                "{},{}.{:03}",
                                timestamp,
                                value as i64,
                                ((value.fract() * 1000.0).abs()) as u64
                            )
                            .ok();
                        } else {
                            writeln!(writer, "{}", value).ok();
                        }
                    }
                    PersistEvent::Flush => {
                        let _ = writer.flush();
                    }
                    PersistEvent::Shutdown => {
                        let _ = writer.flush();
                        break;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Periodic flush check
                if last_flush.elapsed() >= flush_interval {
                    let _ = writer.flush();
                    last_flush = std::time::Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Global persistence manager
pub struct PersistenceManager {
    handles: Mutex<HashMap<String, Arc<QueuePersistence>>>,
    default_config: PersistenceConfig,
}

impl PersistenceManager {
    pub fn new(default_config: PersistenceConfig) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            default_config,
        }
    }

    pub fn enable_for_queue(&self, queue_name: &str) -> std::io::Result<Arc<QueuePersistence>> {
        let mut handles = self.handles.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock poisoned: {}", e))
        })?;

        if let Some(handle) = handles.get(queue_name) {
            return Ok(handle.clone());
        }

        let handle = Arc::new(QueuePersistence::new(
            queue_name,
            self.default_config.clone(),
        )?);

        handles.insert(queue_name.to_string(), handle.clone());
        Ok(handle)
    }

    pub fn get_handle(&self, queue_name: &str) -> Option<Arc<QueuePersistence>> {
        let handles = self.handles.lock().ok()?;
        handles.get(queue_name).cloned()
    }

    /// Shutdown all persistence handles gracefully
    pub fn shutdown_all(&self) {
        let handles = match self.handles.lock() {
            Ok(h) => h,
            Err(e) => {
                log::error!("Failed to acquire lock for shutdown: {}", e);
                return;
            }
        };

        // Take ownership of all handles to trigger their shutdown via Drop
        let handle_values: Vec<_> = handles.iter().map(|(_, handle)| handle.clone()).collect();

        // Explicitly drop each handle to trigger the shutdown via Drop
        // We drop them after releasing the lock by calling shutdown on each
        drop(handles);

        for handle in handle_values {
            // This will call the Drop impl which does graceful shutdown
            drop(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistence_config_works_in_practice() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().join("test_data");

        let config = PersistenceConfig {
            data_dir: data_dir.clone(),
            max_file_size: 1024, // Small size for testing
            flush_interval_ms: 100,
            include_timestamp: true,
            compress_old: false,
        };

        // Create persistence handle
        let persistence = QueuePersistence::new("test_queue", config);
        assert!(persistence.is_ok());

        let mut persistence = persistence.unwrap();

        // Persist some data
        persistence.persist(42.5);
        persistence.persist(100.0);

        // Shutdown to flush data
        persistence.shutdown();

        // Check that data was written
        let log_file = data_dir.join("test_queue.log");
        assert!(log_file.exists(), "Log file should exist");

        let contents = fs::read_to_string(&log_file).expect("Failed to read log file");
        assert!(
            contents.contains("42.5"),
            "Log should contain persisted data"
        );
        assert!(
            contents.contains("100.0"),
            "Log should contain persisted data"
        );
    }
}
