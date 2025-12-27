//! Queue trait definition following KISS principle
//!
//! Ultra-minimal trait with 4 essential methods for maximum flexibility.



/// Simple queue errors
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is empty")]
    Empty,
    
    #[error("Publish error: {0}")]
    PublishError(String),
    
    #[error("Server error: {0}")]
    ServerError(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

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
