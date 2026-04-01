//! Stress testing suite for production load scenarios
//!
//! Tests for high-concurrency workloads, sustained operations,
//! and resource exhaustion scenarios.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ddl::cluster::lock_utils::{RecoverableLock, SafeLock, Validate, ValidationError};
use ddl::cluster::ownership_machine::{OwnershipCommand, OwnershipState};

/// Simple counter for stress tracking
#[derive(Debug)]
struct StressCounter {
    count: AtomicU64,
    operations: AtomicUsize,
    errors: AtomicUsize,
}

impl Clone for StressCounter {
    fn clone(&self) -> Self {
        Self {
            count: AtomicU64::new(self.count.load(Ordering::SeqCst)),
            operations: AtomicUsize::new(self.operations.load(Ordering::SeqCst)),
            errors: AtomicUsize::new(self.errors.load(Ordering::SeqCst)),
        }
    }
}

impl Validate for StressCounter {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Chaos state for failure injection testing
#[derive(Debug, Default)]
struct ChaosState {
    operation_count: AtomicU64,
    failure_injected: AtomicU64,
}

impl Clone for ChaosState {
    fn clone(&self) -> Self {
        Self {
            operation_count: AtomicU64::new(self.operation_count.load(Ordering::SeqCst)),
            failure_injected: AtomicU64::new(self.failure_injected.load(Ordering::SeqCst)),
        }
    }
}

#[test]
fn test_high_concurrent_topic_creation() {
    // Test 100 threads creating topics concurrently

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let mut handles = vec![];

    for i in 0..100 {
        let state_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            let mut guard = state_clone.write_recover("create_topic_thread").unwrap();
            guard.apply(&OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: i as u64,
                timestamp: 1000 + i as u64,
            });
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify state
    let state = ownership_state.read_recover().unwrap();
    assert!(
        state.topic_count() >= 100,
        "Should have at least 100 topics"
    );
}

#[test]
fn test_sustained_high_throughput() {
    // Test sustained operations over time

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let operation_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let duration = Duration::from_millis(100); // Short duration for test
    let start = Instant::now();

    let mut handles = vec![];

    for i in 0..50 {
        let state_clone = Arc::clone(&ownership_state);
        let op_count = Arc::clone(&operation_count);
        let err_count = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            let mut local_count = 0;
            while start.elapsed() < duration {
                if let Ok(mut guard) = state_clone.write_recover("sustained_thread") {
                    guard.apply(&OwnershipCommand::ClaimTopic {
                        topic: format!("topic.{}.{}", i, local_count),
                        node_id: i as u64,
                        timestamp: start.elapsed().as_millis() as u64,
                    });
                    op_count.fetch_add(1, Ordering::Relaxed);
                    local_count += 1;
                } else {
                    err_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let total_ops = operation_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);

    println!(
        "Sustained test: {} operations, {} errors",
        total_ops, errors
    );
    assert!(total_ops > 0, "Should have performed operations");
}

#[test]
fn test_rapid_lease_operations() {
    // Stress test lease acquire/renew/release

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let mut handles = vec![];

    for i in 0..30 {
        let state_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            // Acquire lease
            if let Ok(mut guard) = state_clone.write_recover("acquire_thread") {
                guard.apply(&OwnershipCommand::AcquireLease {
                    node_id: i as u64,
                    ttl_seconds: 60,
                    timestamp: 1000,
                });
            }

            // Quick renew
            if let Ok(mut guard) = state_clone.write_recover("renew_thread") {
                guard.apply(&OwnershipCommand::RenewLease {
                    node_id: i as u64,
                    ttl_seconds: 60,
                    timestamp: 2000,
                });
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify leases exist
    let state = ownership_state.read_recover().unwrap();
    assert!(state.lease_count() > 0, "Should have active leases");
}

#[test]
fn test_production_simulation() {
    // Simulate production load patterns

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    // Simulate producers
    for i in 0..20 {
        let state_clone = Arc::clone(&ownership_state);
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            for j in 0..50 {
                match state_clone.write_recover("producer_thread") {
                    Ok(mut guard) => {
                        guard.apply(&OwnershipCommand::ClaimTopic {
                            topic: format!("prod.{}.{}", i, j),
                            node_id: i as u64,
                            timestamp: j as u64,
                        });
                        success.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        handles.push(handle);
    }

    // Simulate consumers (read-only)
    for _ in 0..10 {
        let state_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if let Ok(_guard) = state_clone.read_recover() {
                    // Read operations
                }
                thread::sleep(Duration::from_micros(100));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total = success + errors;

    println!(
        "Production simulation: {} total, {} success, {} errors",
        total, success, errors
    );
    assert!(success > 0, "Should have successful operations");
}

#[test]
fn test_rapid_lock_recovery() {
    // Test rapid lock acquisition and recovery under stress

    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    let mut handles = vec![];

    for _ in 0..20 {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = lock_clone.write_recover("rapid_thread");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify lock is still functional
    let _guard = lock.read_recover().unwrap();
}

#[test]
fn test_large_state_transitions() {
    // Test handling of large state changes

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let iteration_count = 100;

    // Populate with many topics
    for i in 0..iteration_count {
        let mut guard = ownership_state.write_recover("populate_thread").unwrap();
        guard.apply(&OwnershipCommand::ClaimTopic {
            topic: format!("topic_{}", i),
            node_id: i as u64,
            timestamp: 1000 + i as u64,
        });
    }

    // Verify final state
    let state = ownership_state.read_recover().unwrap();
    assert_eq!(
        state.topic_count(),
        iteration_count,
        "All topics should exist"
    );
}

#[test]
fn test_lock_contention_under_stress() {
    // Test behavior under high lock contention

    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    let mut handles = vec![];

    // Many threads trying to write simultaneously
    for i in 0..50 {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            for _ in 0..20 {
                if let Ok(_guard) = lock_clone.write_recover("contention_thread") {
                    // Simulate work
                    thread::sleep(Duration::from_micros(10));
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify lock is still functional
    let _guard = lock.read_recover().unwrap();
}
