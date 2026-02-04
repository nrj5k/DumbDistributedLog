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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Send to background thread - don't block on failure
        let _ = self.sender.send(PersistEvent::Data { timestamp, value });
    }

    /// Request flush (best effort)
    pub fn flush(&self) {
        let _ = self.sender.send(PersistEvent::Flush);
    }

    /// Graceful shutdown with flush - call this before dropping
    pub fn shutdown(&mut self) {
        // Send shutdown signal
        let _ = self.sender.send(PersistEvent::Shutdown);

        // Wait for thread to finish (with timeout)
        if let Some(handle) = self.handle.take() {
            let _ = handle.join(); // Wait for flush to complete
        }
    }
}

impl Drop for QueuePersistence {
    fn drop(&mut self) {
        // If not already shut down gracefully, do best effort
        if self.handle.is_some() {
            let _ = self.sender.send(PersistEvent::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
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
        let mut handles = self.handles.lock().unwrap();

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
        let handles = self.handles.lock().unwrap();
        handles.get(queue_name).cloned()
    }

    /// Shutdown all persistence handles gracefully
    pub fn shutdown_all(&self) {
        let mut handles = self.handles.lock().unwrap();
        // Take ownership of all handles to trigger their shutdown via Drop
        let handle_values: Vec<_> = handles.drain().map(|(_, handle)| handle).collect();

        // Explicitly drop each handle to trigger the shutdown via Drop
        for handle in handle_values {
            drop(handle);
        }
    }
}

/// Helper function to convert values to f64 for persistence
pub fn convert_to_f64<T>(value: &T) -> Option<f64>
where
    T: 'static,
{
    // Try direct casting for numeric types
    if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<f64>() {
        Some(*v)
    } else if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<f32>() {
        Some(*v as f64)
    } else if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<i64>() {
        Some(*v as f64)
    } else if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<i32>() {
        Some(*v as f64)
    } else if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<u64>() {
        Some(*v as f64)
    } else if let Some(v) = (value as &dyn std::any::Any).downcast_ref::<u32>() {
        Some(*v as f64)
    } else {
        // For non-numeric types, we can't persist without a custom serialization
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_f64() {
        let f64_val = 42.5_f64;
        assert_eq!(convert_to_f64(&f64_val), Some(42.5));

        let f32_val = 42.5_f32;
        assert_eq!(convert_to_f64(&f32_val), Some(42.5));

        let i64_val = 42_i64;
        assert_eq!(convert_to_f64(&i64_val), Some(42.0));

        let string_val = "hello";
        assert_eq!(convert_to_f64(&string_val), None);
    }

    #[test]
    fn test_persistence_config_default() {
        let config = PersistenceConfig::default();
        assert_eq!(config.data_dir, PathBuf::from("data"));
        assert_eq!(config.max_file_size, 100 * 1024 * 1024);
        assert_eq!(config.flush_interval_ms, 1000);
        assert_eq!(config.include_timestamp, true);
        assert_eq!(config.compress_old, false);
    }
}
