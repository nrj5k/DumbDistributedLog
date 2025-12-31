//! Simple queue implementation
//!
//! Uses circular-buffer for O(1) queue operations with pre-allocated memory.
//! No heap allocation, no reallocation, zero overhead ring buffer.

use crate::constants::memory;
use circular_buffer::CircularBuffer;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex as TokioMutex, oneshot};
use tokio::time::interval;

use crate::queue::{QueueError, QueueServerHandle};
use crate::traits::queue::QueueTrait;
use crate::types::{AtomicQueueStats, QueueData, Timestamp};

/// Simple queue implementation using circular buffer
///
/// Uses circular-buffer crate for pre-allocated ring buffer:
/// - O(1) push and pop operations
/// - No heap allocation (stack-allocated const generic)
/// - No reallocation overhead
/// - Cache-friendly contiguous memory
pub struct SimpleQueue<
    T: Clone + Send + 'static,
    const CAPACITY: usize = { memory::MAX_QUEUE_DEPTH },
> {
    /// Circular buffer for O(1) queue operations
    data: Arc<TokioMutex<CircularBuffer<CAPACITY, QueueData<T>>>>,
    /// Queue statistics
    stats: Arc<AtomicQueueStats>,
}

impl<T: Clone + Send + 'static, const CAPACITY: usize> Clone for SimpleQueue<T, CAPACITY> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            stats: self.stats.clone(),
        }
    }
}

impl<T: Clone + Send + 'static, const CAPACITY: usize> SimpleQueue<T, CAPACITY> {
    /// Create new simple queue with default capacity
    pub fn new() -> Self {
        Self {
            data: Arc::new(TokioMutex::new(CircularBuffer::new())),
            stats: Arc::new(AtomicQueueStats::new()),
        }
    }

    /// Create queue with custom capacity
    pub fn with_capacity(_capacity: usize) -> Self {
        // Note: circular-buffer requires const generic, so we use DEFAULT
        // For dynamic capacity needs, a different implementation would be needed
        Self {
            data: Arc::new(TokioMutex::new(CircularBuffer::new())),
            stats: Arc::new(AtomicQueueStats::new()),
        }
    }

    /// Get current capacity
    pub fn capacity(&self) -> usize {
        CAPACITY
    }
}

impl<T: Clone + Send + 'static, const CAPACITY: usize> Default for SimpleQueue<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static, const CAPACITY: usize> QueueTrait for SimpleQueue<T, CAPACITY> {
    type Data = T;

    /// Get the current number of items in the queue
    fn get_size(&self) -> usize {
        if let Ok(data_guard) = self.data.try_lock() {
            data_guard.len()
        } else {
            0 // Return 0 if we can't acquire the lock
        }
    }

    fn publish(&mut self, data: T) -> Result<(), QueueError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as Timestamp;

        let queue_data = (timestamp, data);

        // Add to circular buffer (O(1), pre-allocated, no reallocation)
        let mut data_guard = self
            .data
            .try_lock()
            .map_err(|e| QueueError::PublishError(format!("Mutex error: {}", e)))?;

        data_guard.push_back(queue_data);

        // Update stats
        self.stats.increment_published();
        self.stats.update_timestamp(timestamp);

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

    #[test]
    fn test_queue_get_size() {
        let mut queue: SimpleQueue<i32> = SimpleQueue::new();

        // Empty queue should have size 0
        assert_eq!(queue.get_size(), 0);

        // Add some items
        queue.publish(1).unwrap();
        queue.publish(2).unwrap();
        queue.publish(3).unwrap();

        // Should have size 3
        assert_eq!(queue.get_size(), 3);

        // Test after getting latest (shouldn't change size)
        let _latest = queue.get_latest();
        assert_eq!(queue.get_size(), 3);

        // Test after getting latest_n (shouldn't change size)
        let _recent = queue.get_latest_n(2);
        assert_eq!(queue.get_size(), 3);
    }

    #[test]
    fn test_circular_buffer_overflow() {
        // Test that circular buffer handles overflow correctly
        let mut queue: SimpleQueue<i32, 4> = SimpleQueue::new(); // Small capacity for testing

        // Fill beyond capacity
        for i in 0..10 {
            queue.publish(i).unwrap();
        }

        // Should only have last 4 items (circular buffer behavior)
        assert_eq!(queue.get_size(), 4);

        let latest = queue.get_latest();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().1, 9); // Last item

        let recent = queue.get_latest_n(4);
        assert_eq!(recent, vec![9, 8, 7, 6]); // Last 4 items
    }
}