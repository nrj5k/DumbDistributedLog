//! Queue trait definition following KISS principle
//!
//! Ultra-minimal trait with 4 essential methods for maximum flexibility.

use crate::types::Timestamp;

/// Simple queue errors
#[derive(Debug)]
pub enum QueueError {
    Empty,
    PublishError(String),
    ServerError(String),
    Other(String),
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::Empty => write!(f, "Queue is empty"),
            QueueError::PublishError(msg) => write!(f, "Publish error: {}", msg),
            QueueError::ServerError(msg) => write!(f, "Server error: {}", msg),
            QueueError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for QueueError {}

/// Queue server handle for lifecycle management
pub struct QueueServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl QueueServerHandle {
    /// Create new server handle
    pub fn new(
        shutdown_tx: tokio::sync::oneshot::Sender<()>,
        join_handle: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self {
            shutdown_tx,
            join_handle,
        }
    }

    /// Shutdown the server
    pub async fn shutdown(self) -> Result<(), QueueError> {
        let _ = self.shutdown_tx.send(());
        match self.join_handle.await {
            Ok(_) => Ok(()),
            Err(e) => Err(QueueError::ServerError(e.to_string())),
        }
    }
}
