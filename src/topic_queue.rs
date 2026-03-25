//! Shared TopicQueue implementation for DDL backends
//!
//! Ring buffer for storing entries with subscriber notification.
//! Used by both in-memory and distributed DDL implementations.

use crate::traits::ddl::{DdlError, Entry};
use crossbeam::channel::{Sender, TrySendError};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Subscriber metadata
///
/// Tracks the sender channel and optionally when the subscription was created.
/// `created_at` is used by distributed implementations for monitoring/debugging.
pub struct SubscriberInfo {
    pub sender: Sender<Entry>,
    pub created_at: Option<Instant>,
}

/// Per-topic queue with ring buffer semantics.
///
/// # Mutex Trade-off Design Decision
///
/// This struct uses a hybrid synchronization approach:
/// - `AtomicU64` for `write_pos` and `ack_pos` (lock-free)
/// - `Mutex<Vec<Option<Entry>>>` for `entries` (lock-based)
///
/// ## Why Mutex is Used for Entries
///
/// The entries array requires mutable access for both push (write) and read operations.
/// Rust's ownership model doesn't allow safe lock-free mutable access to a `Vec` without
/// complex unsafe code that would be difficult to verify for correctness and freedom from
/// undefined behavior.
///
/// A fully lock-free implementation would require either:
/// - `UnsafeCell<Entry>` with manual memory ordering guarantees
/// - Atomic pointers with proper lifetime management
/// - A lock-free queue data structure like `crossbeam-queue`
///
/// All of these approaches introduce significant complexity and subtle correctness requirements.
///
/// ## Trade-off Analysis
///
/// **Pros:**
/// - Simplicity: Correct implementation without undefined behavior
/// - Safety: Mutex guarantees exclusive access, preventing data races
/// - Mutex overhead is O(1) for lock/unlock, not proportional to data size
/// - Well-understood semantics and debugging characteristics
///
/// **Cons:**
/// - Contention when multiple producers push to the same topic
/// - Blocks readers during push and vice versa
/// - Lock acquisition overhead in high-contention scenarios
///
/// ## Performance Characteristics
///
/// - Mutex contention is minimal in typical single-producer scenarios
/// - The lock is held briefly (just for index calculation and entry write)
/// - No lock is needed for `ack_pos` updates (atomic)
/// - For multi-producer, consider using per-producer queues or sharding
///
/// ## Future Optimization Paths
///
/// If contention becomes measurable in production:
/// 1. Use `crossbeam-queue` for fully lock-free implementation
/// 2. Use `Box<[UnsafeCell<AtomicPtr<Entry>>]>` with proper synchronization
///    (requires careful memory ordering and lifetime management)
/// 3. Use sharding by topic hash to distribute across multiple queues
/// 4. Consider `parking_lot::Mutex` for potentially better performance
///
/// Align to cache line boundary to prevent false sharing.
#[repr(align(64))]
pub struct TopicQueue {
    /// Write position (monotonically increasing) - placed first for cache alignment
    write_pos: AtomicU64,
    /// Last acknowledged position
    ack_pos: AtomicU64,
    /// Entries stored in ring buffer
    entries: Mutex<Vec<Option<Entry>>>,
    /// Buffer size (power of 2 for efficient wrap-around)
    capacity: usize,
    /// Mask for efficient modulo operations
    mask: usize,
}

impl TopicQueue {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;

        let mut entries = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            entries.push(None);
        }

        Self {
            write_pos: AtomicU64::new(0),
            ack_pos: AtomicU64::new(0),
            entries: Mutex::new(entries),
            capacity,
            mask,
        }
    }

    /// Append an entry, returns the entry ID
    pub fn push(&self, topic: Arc<str>, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Convert Vec<u8> to Arc<[u8]> for zero-copy sharing
        let payload_arc = Arc::from(payload);

        // Lock entries first - this provides mutual exclusion for both
        // the write_pos increment and the entry write, ensuring atomicity
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| DdlError::LockPoisoned(topic.to_string()))?;

        // Get current position inside the lock
        let id = self.write_pos.load(Ordering::Relaxed);
        let index = (id as usize) & self.mask;

        // Check if we're overwriting unacknowledged data
        let ack = self.ack_pos.load(Ordering::Acquire);
        if id >= ack + self.capacity as u64 {
            return Err(DdlError::BufferFull(topic.to_string()));
        }

        let entry = Entry {
            id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            topic,
            payload: payload_arc,
        };

        // Store the entry BEFORE incrementing write_pos
        // This ensures readers can't see incomplete entries
        entries[index] = Some(entry);

        // Now make the entry visible by incrementing write_pos with Release ordering
        self.write_pos.store(id + 1, Ordering::Release);

        Ok(id)
    }

    /// Read entry at specific ID
    /// Uses Acquire ordering to ensure we see the complete entry
    pub fn read(&self, id: u64) -> Option<Entry> {
        // Load the write position with Acquire ordering to synchronize with Release in push
        let write_pos = self.write_pos.load(Ordering::Acquire);

        // Check if this ID exists
        if id >= write_pos {
            return None;
        }

        let index = (id as usize) & self.mask;
        if let Ok(entries) = self.entries.lock() {
            entries[index].clone()
        } else {
            log::warn!(
                "Poisoned lock on topic queue for entry id {}, returning None",
                id
            );
            None
        }
    }

    /// Acknowledge entries up to this ID
    pub fn ack(&self, id: u64) {
        // Simple: just move ack_pos forward with relaxed ordering
        // In production: handle out-of-order acks
        let current = self.ack_pos.load(Ordering::Acquire);
        if id > current {
            self.ack_pos.store(id, Ordering::Release);
        }
    }

    /// Get current position (next ID to write)
    pub fn position(&self) -> u64 {
        self.write_pos.load(Ordering::Relaxed)
    }
}

/// Notify subscribers of a new entry with backpressure handling
pub fn notify_subscribers(
    subscribers: &DashMap<String, Vec<SubscriberInfo>>,
    topic: &str,
    entry: &Entry,
    backpressure: crate::traits::ddl::BackpressureMode,
) -> Result<(), DdlError> {
    if let Some(senders) = subscribers.get(topic) {
        for info in senders.value().iter() {
            match backpressure {
                crate::traits::ddl::BackpressureMode::Block => {
                    // This should not be called in async context, but we handle it
                    // In practice, Block mode requires async send
                    if let Err(e) = info.sender.send(entry.clone()) {
                        log::debug!("Subscriber disconnected: {:?}", e);
                    }
                }
                crate::traits::ddl::BackpressureMode::DropOldest => {
                    // Try to send, drop if full (non-blocking)
                    match info.sender.try_send(entry.clone()) {
                        Err(TrySendError::Full(_)) => {
                            log::debug!(
                                "Subscriber buffer full for topic {}, dropping message",
                                topic
                            );
                        }
                        Err(TrySendError::Disconnected(_)) => {
                            log::debug!("Subscriber disconnected for topic {}", topic);
                        }
                        Ok(_) => {}
                    }
                }
                crate::traits::ddl::BackpressureMode::DropNewest => {
                    // Check if full before sending
                    if info.sender.is_full() {
                        log::debug!(
                            "Subscriber buffer full for topic {}, dropping newest",
                            topic
                        );
                        continue;
                    }
                    if let Err(e) = info.sender.try_send(entry.clone()) {
                        log::debug!("Send error for topic {}: {:?}", topic, e);
                    }
                }
                crate::traits::ddl::BackpressureMode::Error => {
                    // Return error if full
                    if info.sender.is_full() {
                        return Err(DdlError::SubscriberBufferFull(topic.to_string()));
                    }
                    if let Err(e) = info.sender.try_send(entry.clone()) {
                        log::debug!("Send error for topic {}: {:?}", topic, e);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn test_concurrent_push_pop() {
        let queue = Arc::new(TopicQueue::new(1024));
        let barrier = Arc::new(Barrier::new(11)); // 10 producers + 1 consumer

        let mut handles = vec![];

        // Spawn 10 producers
        for i in 0..10 {
            let q = queue.clone();
            let b = barrier.clone();
            handles.push(thread::spawn(move || {
                b.wait(); // Synchronize all threads to start simultaneously
                for j in 0..100 {
                    q.push(format!("entry_{}_{}", i, j).into(), vec![j as u8; 64])
                        .unwrap();
                }
            }));
        }

        // Spawn consumer
        let q = queue.clone();
        let b = barrier.clone();
        let consumer = thread::spawn(move || {
            b.wait();
            let mut count = 0u64;
            let mut id = 0u64;

            // Read until we've got all 1000 messages
            while count < 1000 {
                // Try to read next expected entry
                if let Some(entry) = q.read(id) {
                    // Verify entry is complete and valid
                    assert_eq!(entry.id, id, "Entry ID mismatch at {}", id);
                    assert!(!entry.payload.is_empty(), "Empty payload at {}", id);
                    assert_eq!(entry.payload.len(), 64, "Payload size mismatch at {}", id);
                    count += 1;
                    id += 1;
                }
                // Small yield to avoid tight spinning
                if count % 100 == 0 {
                    thread::yield_now();
                }
            }
            count
        });

        for h in handles {
            h.join().unwrap();
        }

        let count = consumer.join().unwrap();
        assert_eq!(count, 1000, "Should have read all 1000 entries");

        // Verify final queue position
        assert_eq!(queue.position(), 1000);
    }
}
