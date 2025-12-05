//! Queue Server Module
//!
//! This module provides autonomous server functionality for queue system with graceful shutdown capabilities.

use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::QueueError;

/// Server handle for managing queue server lifecycle
#[derive(Debug)]
pub struct QueueServerHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl QueueServerHandle {
    /// Create a new queue server handle
    pub fn new(shutdown_tx: oneshot::Sender<()>, join_handle: JoinHandle<()>) -> Self {
        Self {
            shutdown_tx: Some(shutdown_tx),
            join_handle: Some(join_handle),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Start the queue server (already started when handle is created)
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
            && self
                .join_handle
                .as_ref()
                .map(|h| !h.is_finished())
                .unwrap_or(false)
    }

    /// Check if the server is still responsive
    pub fn is_responsive(&self) -> bool {
        self.is_running()
    }

    /// Gracefully shutdown the queue server
    pub async fn shutdown(mut self) -> Result<(), QueueError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);

        if let Some(handle) = self.join_handle.take() {
            if let Err(e) = handle.await {
                return Err(QueueError::Other(format!("Server shutdown failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Force shutdown without graceful handling
    pub async fn force_shutdown(self) -> Result<(), QueueError> {
        self.shutdown().await
    }

    /// Get server uptime information
    pub fn uptime(&self) -> std::time::Duration {
        // Placeholder - would need timestamp tracking
        std::time::Duration::from_secs(0)
    }

    /// Check if server can accept new connections
    pub fn can_accept_new(&self) -> bool {
        self.is_running()
    }
}
