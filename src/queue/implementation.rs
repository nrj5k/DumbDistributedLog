//! Queue implementation following KISS principle
//!
//! Moved from core.rs to new structure with singular objective:
//! - Queue data management and operations
//! - Server lifecycle management
//! - Integration with expression system

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex as TokioMutex, oneshot};
use tokio::time::interval;

use super::{Queue, QueueError, QueueServerHandle};
use crate::expression::Expression;
use crate::types::{QueueData, QueueStats, Timestamp};

/// Simple queue implementation
pub struct SimpleQueue<T: Clone + Send + 'static> {
    data: Arc<TokioMutex<VecDeque<QueueData<T>>>>,
    expression: Option<Box<dyn Expression>>,
    stats: Arc<TokioMutex<QueueStats>>,
}

impl<T: Clone + Send + 'static> Clone for SimpleQueue<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            expression: None, // Can't clone Expression trait objects
            stats: self.stats.clone(),
        }
    }
}

impl<T: Clone + Send + 'static> SimpleQueue<T> {
    /// Create new simple queue
    pub fn new() -> Self {
        Self {
            data: Arc::new(TokioMutex::new(VecDeque::new())),
            expression: None,
            stats: Arc::new(TokioMutex::new(QueueStats::new())),
        }
    }

    /// Create queue with expression
    pub fn with_expression(expr: Box<dyn Expression>) -> Self {
        Self {
            data: Arc::new(TokioMutex::new(VecDeque::new())),
            expression: Some(expr),
            stats: Arc::new(TokioMutex::new(QueueStats::new())),
        }
    }
}

impl<T: Clone + Send + 'static> Queue for SimpleQueue<T> {
    type Data = T;

    fn publish(&mut self, data: T) -> Result<(), QueueError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as Timestamp;

        let queue_data = (timestamp, data);

        // Add to queue (maintain max size)
        let mut data_guard = self
            .data
            .try_lock()
            .map_err(|e| QueueError::PublishError(format!("Mutex error: {}", e)))?;

        data_guard.push_back(queue_data);

        // Update stats
        let mut stats_guard = self
            .stats
            .try_lock()
            .map_err(|e| QueueError::PublishError(format!("Stats mutex error: {}", e)))?;

        stats_guard.total_published += 1;
        stats_guard.last_updated = timestamp;

        Ok(())
    }

    fn get_latest(&self) -> Option<(Timestamp, T)> {
        let data_guard = self.data.try_lock().ok()?;
        data_guard.back().cloned()
    }

    fn get_latest_n(&self, n: usize) -> Vec<T> {
        if let Ok(data_guard) = self.data.try_lock() {
            data_guard
                .iter()
                .rev()
                .take(n)
                .map(|(_, data)| data.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn start_server(self) -> Result<QueueServerHandle, QueueError> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let _data = self.data.clone();
        let _stats = self.stats.clone();

        let join_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(1000));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Process queue data here if needed
                        // For now, just maintain stats
                    }
                    _ = &mut shutdown_rx => {
                        println!("Queue server shutting down");
                        break;
                    }
                }
            }
        });

        Ok(QueueServerHandle::new(shutdown_tx, join_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_queue_creation() {
        let queue: SimpleQueue<i32> = SimpleQueue::new();
        assert_eq!(queue.get_latest(), None);
        assert_eq!(queue.get_latest_n(5), Vec::<i32>::new());
    }

    #[test]
    fn test_queue_publish_and_retrieve() {
        let mut queue: SimpleQueue<String> = SimpleQueue::new();

        queue.publish("test1".to_string()).unwrap();
        queue.publish("test2".to_string()).unwrap();

        let latest = queue.get_latest();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().1, "test2");

        let recent = queue.get_latest_n(2);
        assert_eq!(recent, vec!["test2", "test1"]);
    }
}
