//! Queue Server Module
//!
//! This module provides autonomous server functionality for queue system with graceful shutdown capabilities.

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::enums::QueueError;

/// Server handle for managing queue server lifecycle
pub struct QueueServerHandle {
    shutdown_tx: oneshot::Sender<()>,
    join_handle: JoinHandle<()>,
}

impl QueueServerHandle {
    /// Create a new queue server handle
    pub fn new(shutdown_tx: oneshot::Sender<()>, join_handle: JoinHandle<()>) -> Self {
        Self {
            shutdown_tx,
            join_handle,
        }
    }

    /// Start the queue server (already started when handle is created)
    pub fn is_running(&self) -> bool {
        !self.join_handle.is_finished()
    }

    /// Gracefully shutdown the queue server
    pub async fn shutdown(self) -> Result<(), QueueError> {
        let _ = self.shutdown_tx.send(());
        if let Err(e) = self.join_handle.await {
            return Err(QueueError::Other(format!("Server shutdown failed: {}", e)));
        }
        Ok(())
    }
}
