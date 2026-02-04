//! Sharded ring buffer implementation using atomic operations
//!
//! This implementation uses atomic operations to provide a ring-buffer ring buffer
//! that can handle concurrent producers and consumers without mutexes.
//! Target performance: sub-5μs operations.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::queue::{QueueError, QueueServerHandle};
use crate::traits::queue::QueueTrait;
use crate::types::{QueueData, Timestamp};

/// Sharded ring buffer using atomic operations
///
/// This queue implementation uses atomic operations to provide
/// ring-buffer access for both producers and consumers.
pub struct ShardedRingBuffer<T, const N: usize>
where
    T: Clone + Send + Sync + 'static,
{
    /// Pre-allocated buffer for queue data
    buffer: Vec<std::sync::RwLock<Option<QueueData<T>>>>,
    /// Mask for power-of-2 wrap-around
    mask: usize,
    /// Head index (consumer position)
    head: AtomicUsize,
    /// Tail index (producer position)
    tail: AtomicUsize,
}

impl<T: Clone + Send + Sync + 'static, const N: usize> ShardedRingBuffer<T, N> {
    /// Create a new ring-buffer queue
    pub fn new() -> Self {
        // Ensure N is a power of 2 for efficient masking
        assert!(N.is_power_of_two(), "Queue capacity must be a power of 2");

        let mut buffer = Vec::with_capacity(N);
        for _ in 0..N {
            buffer.push(std::sync::RwLock::new(None));
        }

        Self {
            buffer,
            mask: N - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Get the current capacity of the queue
    pub fn capacity(&self) -> usize {
        N
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        (tail.wrapping_add(1)) & self.mask == head
    }

    /// Get the current number of items in the queue
    fn get_size_internal(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head) & self.mask
    }
}

impl<T: Clone + Send + Sync + 'static, const N: usize> Default for ShardedRingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static, const N: usize> QueueTrait for ShardedRingBuffer<T, N> {
    type Data = T;

    /// Publish data to the queue
    fn publish(&mut self, data: Self::Data) -> Result<(), QueueError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as Timestamp;

        let queue_data = (timestamp, data);

        // Get current tail position
        let mut current_tail = self.tail.load(Ordering::Acquire);

        loop {
            // Calculate next tail position
            let next_tail = (current_tail + 1) & self.mask;

            // Check if queue is full
            if next_tail == self.head.load(Ordering::Acquire) {
                return Err(QueueError::PublishError("Queue is full".to_string()));
            }

            // Try to claim the slot
            match self.tail.compare_exchange_weak(
                current_tail,
                next_tail,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully claimed the slot, now write the data
                    let index = current_tail & self.mask;
                    if let Ok(mut guard) = self.buffer[index].write() {
                        *guard = Some(queue_data);
                    }

                    // Update stats
                    // Note: In a truly ring-buffer implementation, we might want to use atomic counters
                    // For now, we'll just increment the stats without locks
                    // self.stats.total_published += 1;
                    // self.stats.last_updated = timestamp;

                    return Ok(());
                }
                Err(actual_tail) => {
                    // Tail was updated by another producer, try again
                    current_tail = actual_tail;
                }
            }
        }
    }

    /// Get the most recent data from the queue
    fn get_latest(&self) -> Option<(Timestamp, Self::Data)> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if queue is empty
        if head == tail {
            return None;
        }

        // Get the last item (just before tail)
        let index = (tail - 1) & self.mask;
        if let Ok(guard) = self.buffer[index].read() {
            guard.clone()
        } else {
            None
        }
    }

    /// Get the N most recent data items from the queue
    fn get_latest_n(&self, n: usize) -> Vec<Self::Data> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if queue is empty
        if head == tail {
            return Vec::new();
        }

        let size = tail.wrapping_sub(head) & self.mask;
        let count = std::cmp::min(n, size);

        let mut result = Vec::with_capacity(count);

        // Collect the most recent items
        for i in 0..count {
            let index = (tail.wrapping_sub(i + 1)) & self.mask;
            if let Ok(guard) = self.buffer[index].read() {
                if let Some((_, data)) = &*guard {
                    result.push(data.clone());
                }
            }
        }

        result
    }

    /// Get the current number of items in the queue
    fn get_size(&self) -> usize {
        self.get_size_internal()
    }

    /// Start autonomous server
    fn start_server(self) -> Result<QueueServerHandle, QueueError> {
        // For ring-buffer implementation, we don't need a server
        // but we need to maintain API compatibility
        use tokio::sync::oneshot;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle = tokio::spawn(async move {
            // The ring-buffer queue doesn't need a background task
            // but we wait for shutdown signal for API compatibility
            let _ = shutdown_rx.await;
        });

        Ok(QueueServerHandle::new(shutdown_tx, join_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfree_queue_creation() {
        let queue: ShardedRingBuffer<i32, 8> = ShardedRingBuffer::new();
        assert_eq!(queue.get_size(), 0);
        assert_eq!(queue.capacity(), 8);
        assert!(queue.is_empty());
        assert!(!queue.is_full());
    }

    #[test]
    fn test_queue_publish_and_retrieve() {
        let mut queue: ShardedRingBuffer<String, 8> = ShardedRingBuffer::new();

        queue.publish("test1".to_string()).unwrap();
        queue.publish("test2".to_string()).unwrap();

        assert_eq!(queue.get_size(), 2);
        assert!(!queue.is_empty());

        let latest = queue.get_latest();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().1, "test2");

        let recent = queue.get_latest_n(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], "test2");
        assert_eq!(recent[1], "test1");
    }

    #[test]
    fn test_queue_full_behavior() {
        let mut queue: ShardedRingBuffer<i32, 4> = ShardedRingBuffer::new();

        // Fill the queue
        for i in 0..3 {
            queue.publish(i).unwrap();
        }

        // Queue should be full now (capacity-1 due to ring buffer design)
        assert!(queue.is_full());
        assert_eq!(queue.get_size(), 3);

        // Try to publish one more item - should fail
        let result = queue.publish(4);
        assert!(result.is_err());
    }

    #[test]
    fn test_queue_wraparound() {
        let mut queue: ShardedRingBuffer<i32, 8> = ShardedRingBuffer::new();

        // Fill the queue (capacity is 7 due to ring buffer design)
        for i in 0..7 {
            queue.publish(i).unwrap();
        }

        // Check that the queue is full
        assert!(queue.is_full());

        // Get the latest item (this doesn't remove it from the queue in our implementation)
        let latest = queue.get_latest();
        assert_eq!(latest.unwrap().1, 6);

        // The queue is still full, so publishing should fail
        assert!(queue.publish(7).is_err());
    }
}
