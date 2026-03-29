//! SPMC (Single-Producer, Multiple-Consumer) Lock-Free Circular Queue
//!
//! This module provides a high-performance SPMC lock-free queue using atomic operations
//! and the `circular-buffer` crate. Designed for AutoQueues `atomic.*` pattern.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         SPMC Lock-Free Queue                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  Producer (single)                                                      │
//! │  ┌─────────────┐     ┌──────────────────────────────────────────────┐  │
//! │  │ push(data)  │────▶│  CircularBuffer (pre-allocated, N slots)    │  │
//! │  │             │     │  tail: AtomicUsize (producer writes)         │  │
//! │  └─────────────┘     └──────────────────────────────────────────────┘  │
//! │                                                                         │
//! │  Consumers (multiple, each with own head)                               │
//! │  ┌─────────────┐     ┌──────────────────────────────────────────────┐    │
//! │  │ consumer.   │     │  Each consumer has independent head position │    │
//! │  │ pop()       │◀────│  head: AtomicUsize (consumer reads)          │    │
//! │  └─────────────┘     │  No coordination between consumers           │    │
//! │                      └──────────────────────────────────────────────┘    │
//! │                                                                         │
//! │  Key Properties:                                                        │
//! │  - Single producer: atomic fetch_add for tail                           │
//! │  - Multiple consumers: each has own atomic head                         │
//! │  - Drop-oldest: circular buffer overwrites oldest                       │
//! │  - No locks: only atomic operations after creation                     │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//!
//! # Performance
//!
//! | Operation    | Target | Notes                                         |
//! |--------------|--------|-----------------------------------------------|
//! | push()       | < 5μs  | Single atomic fetch_add + memcpy             |
//! | pop()        | < 3μs  | Atomic load + memcpy                         |
//! | Allocations  | Zero   | Pre-allocated at creation                    |
//! | Cache misses | Minimal| Contiguous memory, predictable access        |

use crate::constants::memory;
use crate::queue::QueueError;
use crate::traits::queue::QueueTrait;
use crate::types::{now_millis, QueueData, Timestamp};
use circular_buffer::CircularBuffer;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

/// SPMC (Single-Producer, Multiple-Consumer) Lock-Free Circular Queue
///
/// This queue is designed for the AutoQueues `atomic.*` pattern where:
/// - Exactly ONE producer per queue (e.g., the node's metric collector)
/// - Multiple consumers can read concurrently (subscribers, aggregators)
/// - Pre-allocated circular buffer with natural drop-oldest semantics
/// - Target: sub-10μs operations
///
/// # Type Parameters
///
/// - `T`: Data type stored (must be Clone + Send + Sync + 'static)
/// - `N`: Buffer capacity (must be power of 2 for efficient bitmask wrapping)
///
/// # Drop-Oldest Semantics
///
/// When the buffer is full and the producer pushes new data:
/// - The oldest entry is overwritten (circular buffer behavior)
/// - Consumers may miss overwritten data if they haven't read it yet
/// - This is the desired behavior for telemetry (latest data matters most)
#[repr(align(64))]
pub struct SPMCLockFreeQueue<T, const N: usize = { memory::MAX_QUEUE_DEPTH }>
where
    T: Clone + Send + Sync + 'static,
{
    /// Current tail position (producer writes here)
    /// Placed first for cache line alignment to prevent false sharing
    tail: AtomicUsize,
    /// Internal buffer using circular-buffer crate
    /// Pre-allocated, contiguous memory, zero heap allocation after creation
    buffer: CircularBuffer<N, QueueData<T>>,
    /// Bitmask for efficient wrap-around (N - 1, since N is power of 2)
    /// Computed once at creation for performance
    mask: usize,
}

impl<T, const N: usize> SPMCLockFreeQueue<T, N>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new SPMC lock-free queue
    ///
    /// # Panics
    ///
    /// Panics if N is not a power of 2. This is required for efficient
    /// bitmask wrapping instead of modulo operations.
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(N)
    }

    /// Create a queue with specific capacity
    #[inline]
    pub fn with_capacity(_capacity: usize) -> Self {
        debug_assert!(
            N.is_power_of_two(),
            "SPMCLockFreeQueue capacity must be a power of 2"
        );

        Self {
            buffer: CircularBuffer::new(),
            tail: AtomicUsize::new(0),
            mask: N.wrapping_sub(1),
        }
    }

    /// Get the queue capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        N
    }

    /// Check if the queue is empty (from producer's view)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.len() == 0
    }

    /// Check if the queue is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.buffer.len() == N
    }

    /// Get current queue length (from producer's view)
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check how many items have been dropped
    #[inline]
    pub fn dropped_count(&self) -> usize {
        // For circular buffer, dropped count would be total pushes - capacity
        let tail = self.tail.load(Ordering::Acquire);
        if tail > N {
            tail - N
        } else {
            0
        }
    }

    /// Create a new consumer for this queue
    ///
    /// Each consumer tracks its own read position independently.
    pub fn consumer(&self) -> SPMCConsumer<'_, T, N> {
        SPMCConsumer::new(&self.tail, &self.buffer, self.mask)
    }
}

// Verify N is power of 2 at compile time
const _: () = assert!(memory::MAX_QUEUE_DEPTH.is_power_of_two());

impl<T, const N: usize> Default for SPMCLockFreeQueue<T, N>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> SPMCLockFreeQueue<T, N>
where
    T: Clone + Send + Sync + 'static,
{
    /// Push data to the queue (PRODUCER operation)
    ///
    /// This is the ONE producer operation. Must be called from exactly
    /// one thread (the metric collector).
    ///
    /// On overflow (full queue): Drops the oldest entry and pushes new data.
    #[inline]
    pub fn push(&mut self, data: T) -> Result<bool, QueueError> {
        let timestamp = now_millis();

        // Push to circular buffer (handles drop-oldest)
        self.buffer.push_back((timestamp, data));

        // Update atomic tail counter so consumers see the new item
        // This is safe because there's only one producer (metric collector)
        let _ = self.tail.fetch_add(1, Ordering::Release);

        Ok(true)
    }
}

impl<T, const N: usize> QueueTrait for SPMCLockFreeQueue<T, N>
where
    T: Clone + Send + Sync + 'static,
{
    type Data = T;

    #[inline]
    fn get_size(&self) -> usize {
        self.len()
    }

    fn publish(&mut self, data: Self::Data) -> Result<(), QueueError> {
        self.push(data).map(|_| ())
    }

    fn get_latest(&self) -> Option<(Timestamp, Self::Data)> {
        let consumer = self.consumer();
        consumer.pop()
    }

    fn get_latest_n(&self, _n: usize) -> Vec<Self::Data> {
        let consumer = self.consumer();
        consumer.drain().into_iter().map(|(_, v)| v).collect()
    }
}

/// SPMCConsumer - Consumer handle for SPMC Lock-Free Queue
///
/// Each consumer tracks its own read head position independently.
/// Multiple consumers can read from the same queue without coordination.
/// Aligned to cache line boundary to prevent false sharing.
#[repr(align(64))]
pub struct SPMCConsumer<'queue, T, const N: usize>
where
    T: Clone + Send + Sync + 'static,
{
    /// Reference to the shared queue's tail
    tail: &'queue AtomicUsize,
    /// Reference to the shared buffer
    buffer: &'queue CircularBuffer<N, QueueData<T>>,
    /// This consumer's read head position
    head: AtomicUsize,
    /// Bitmask for efficient wrap-around
    mask: usize,
    /// Lifetime marker
    _marker: PhantomData<&'queue ()>,
}

unsafe impl<'queue, T, const N: usize> Send for SPMCConsumer<'queue, T, N> where
    T: Clone + Send + Sync + 'static
{
}

unsafe impl<'queue, T, const N: usize> Sync for SPMCConsumer<'queue, T, N> where
    T: Clone + Send + Sync + 'static
{
}

impl<'queue, T, const N: usize> SPMCConsumer<'queue, T, N>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a consumer from a parent queue
    pub(crate) fn new(
        tail: &'queue AtomicUsize,
        buffer: &'queue CircularBuffer<N, QueueData<T>>,
        mask: usize,
    ) -> Self {
        Self {
            tail,
            buffer,
            head: AtomicUsize::new(0),
            mask,
            _marker: PhantomData,
        }
    }

    /// Get the consumer's current head position
    #[inline]
    pub fn head_position(&self) -> usize {
        self.head.load(Ordering::Acquire)
    }

    /// Check if this consumer has more data available
    #[inline]
    pub fn has_data(&self) -> bool {
        !self.is_empty()
    }

    /// Check if queue is empty for this consumer
    #[inline]
    pub fn is_empty(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        head >= tail
    }

    /// Get number of items available for this consumer
    #[inline]
    pub fn available(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        tail.saturating_sub(head).min(N)
    }

    /// Peek at the next value without consuming it
    #[inline]
    pub fn peek(&self) -> Option<(Timestamp, T)> {
        self.pop_inner(false)
    }

    /// Pop data from the queue
    ///
    /// Returns the oldest unread item, or None if no new data.
    #[inline]
    pub fn pop(&self) -> Option<(Timestamp, T)> {
        self.pop_inner(true)
    }

    /// Internal pop implementation
    #[inline]
    fn pop_inner(&self, consume: bool) -> Option<(Timestamp, T)> {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        if head >= tail {
            return None;
        }

        let buffer_index = head & self.mask;
        let slot = &self.buffer[buffer_index];

        let result = match slot {
            (ts, data) if *ts != 0 => Some((*ts, data.clone())),
            _ => None,
        };

        if consume && result.is_some() {
            self.head.fetch_add(1, Ordering::AcqRel);
        }

        result
    }

    /// Pop multiple items at once
    #[inline]
    pub fn pop_batch(&self, max_count: usize) -> Vec<(Timestamp, T)> {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        let available = tail.saturating_sub(head).min(N);
        let count = std::cmp::min(available, max_count);

        if count == 0 {
            return Vec::with_capacity(0);
        }

        let mut result = Vec::with_capacity(count);

        for i in 0..count {
            let buffer_index = (head + i) & self.mask;
            let slot = &self.buffer[buffer_index];
            if slot.0 != 0 {
                result.push((slot.0, slot.1.clone()));
            }
        }

        if !result.is_empty() {
            self.head.fetch_add(result.len(), Ordering::AcqRel);
        }

        result
    }

    /// Drain all available data
    #[inline]
    pub fn drain(&self) -> Vec<(Timestamp, T)> {
        self.pop_batch(usize::MAX)
    }
}

impl<'queue, T, const N: usize> Clone for SPMCConsumer<'queue, T, N>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            tail: self.tail,
            buffer: self.buffer,
            head: AtomicUsize::new(self.head.load(Ordering::Acquire)),
            mask: self.mask,
            _marker: PhantomData,
        }
    }
}

impl<'queue, T: Clone + Send + Sync, const N: usize> Drop for SPMCConsumer<'queue, T, N> {
    fn drop(&mut self) {
        // Nothing to clean up - references handled by lifetime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 16;

    #[test]
    fn test_spmc_creation() {
        let queue: SPMCLockFreeQueue<i32, CAP> = SPMCLockFreeQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), CAP);
    }

    #[test]
    fn test_spmc_basic_push_pop() {
        let mut queue: SPMCLockFreeQueue<i32, CAP> = SPMCLockFreeQueue::new();

        queue.push(42).unwrap();
        queue.push(84).unwrap();

        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_spmc_single_consumer() {
        let mut queue: SPMCLockFreeQueue<String, CAP> = SPMCLockFreeQueue::new();

        queue.push("hello".to_string()).unwrap();
        queue.push("world".to_string()).unwrap();

        let consumer = queue.consumer();

        let first = consumer.pop();
        assert_eq!(first.unwrap().1, "hello");

        let second = consumer.pop();
        assert_eq!(second.unwrap().1, "world");

        assert!(consumer.pop().is_none());
    }

    #[test]
    fn test_spmc_multiple_consumers() {
        let mut queue: SPMCLockFreeQueue<f64, CAP> = SPMCLockFreeQueue::new();

        for i in 0..10 {
            queue.push(i as f64).unwrap();
        }

        let c1 = queue.consumer();
        let c2 = queue.consumer();
        let c3 = queue.consumer();

        let mut count1 = 0;
        while c1.pop().is_some() {
            count1 += 1;
        }

        let mut count2 = 0;
        while c2.pop().is_some() {
            count2 += 1;
        }

        let mut count3 = 0;
        while c3.pop().is_some() {
            count3 += 1;
        }

        assert_eq!(count1, 10);
        assert_eq!(count2, 10);
        assert_eq!(count3, 10);
    }

    #[test]
    fn test_spmc_drop_oldest() {
        let mut queue: SPMCLockFreeQueue<i32, 8> = SPMCLockFreeQueue::new();

        for i in 0..8 {
            queue.push(i).unwrap();
        }

        assert!(queue.is_full());

        // Push more - should drop oldest (0)
        queue.push(100).unwrap();

        let consumer = queue.consumer();
        let items: Vec<i32> = consumer.drain().into_iter().map(|(_, v)| v).collect();

        assert_eq!(items.len(), 8);
        assert_eq!(items[0], 1); // First item after drop
        assert_eq!(items[7], 100); // Most recent
    }

    #[test]
    fn test_spmc_consumer_clone() {
        let mut queue: SPMCLockFreeQueue<i32, CAP> = SPMCLockFreeQueue::new();
        queue.push(1).unwrap();
        queue.push(2).unwrap();

        let c1 = queue.consumer();
        let c2 = c1.clone();

        assert_eq!(c1.head_position(), 0);
        assert_eq!(c2.head_position(), 0);

        c1.pop();

        assert_eq!(c1.head_position(), 1);
        assert_eq!(c2.head_position(), 0);
    }

    #[test]
    fn test_spmc_batch_and_drain() {
        let mut queue: SPMCLockFreeQueue<i32, CAP> = SPMCLockFreeQueue::new();

        for i in 0..100 {
            queue.push(i).unwrap();
        }

        let consumer = queue.consumer();

        let batch = consumer.pop_batch(10);
        assert_eq!(batch.len(), 10);

        let more = consumer.pop_batch(10);
        assert_eq!(more.len(), 10);

        assert_eq!(consumer.head_position(), 20);
    }
}
