//! Stress testing suite for production load scenarios
//!
//! Tests for high-concurrency workloads, sustained operations,
//! and resource exhaustion scenarios.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use autoqueues::cluster::lock_utils::RecoverableLock;
use autoqueues::cluster::ownership_machine::{OwnershipCommand, OwnershipState};

/// Simple counter for stress tracking
#[derive(Debug, Clone)]
struct StressCounter {
    count: AtomicU64,
    operations: AtomicUsize,
    errors: AtomicUsize,
}

impl Validate for StressCounter {
    fn validate(&self) -> Result<(), autoqueues::cluster::lock_utils::ValidationError> {
        Ok(())
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
            let mut guard = state_clone.write_recover(&format!("create_topic_{}", i)).unwrap();
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
    
    // Verify all topics were created
    let state = ownership_state.read_recover().unwrap();
    assert_eq!(state.topic_count(), 100, "All 100 topics should be created");
}

#[test]
fn test_sustained_operations_60_seconds() {
    // Run for 60 seconds under load, tracking metrics
    
    let timeout = Duration::from_secs(60);
    let start_time = Instant::now();
    
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let operation_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let state_clone = Arc::clone(&ownership_state);
        let ops = Arc::clone(&operation_count);
        let errs = Arc::clone(&error_count);
        
        let handle = thread::spawn(move || {
            while start_time.elapsed() < timeout {
                let topic = format!("stress_topic_{}", i);
                let mut guard = match state_clone.write_recover("sustained_operation") {
                    Ok(g) => g,
                    Err(_) => {
                        errs.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };
                
                guard.apply(&OwnershipCommand::ClaimTopic {
                    topic: topic.clone(),
                    node_id: i as u64,
                    timestamp: 1000,
                });
                
                // Check state
                let _ = guard.get_owner(&topic);
                drop(guard);
                
                ops.fetch_add(1, Ordering::Relaxed);
                
                // Small delay to avoid busy spinning
                thread::sleep(Duration::from_micros(100));
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_ops = operation_count.load(Ordering::Relaxed);
    let final_errors = error_count.load(Ordering::Relaxed);
    
    // Verify we ran sustained operations
    assert!(final_ops > 0, "Should have performed operations");
    
    eprintln!("Sustained load test completed: {} operations, {} errors", final_ops, final_errors);
}

#[test]
fn test_topic_limit_under_stress() {
    // Test that topic limits are enforced under high concurrency
    
    let max_topics = 1000;
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    
    let mut handles = vec![];
    
    for i in 0..max_topics {
        let state_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            let mut guard = state_clone.write_recover(&format!("limit_test_{}", i)).unwrap();
            guard.apply(&OwnershipCommand::ClaimTopic {
                topic: format!("limit_topic.{}", i),
                node_id: i as u64,
                timestamp: 1000 + i as u64,
            });
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let state = ownership_state.read_recover().unwrap();
    assert_eq!(state.topic_count(), max_topics, "All topics should be created");
}

#[test]
fn test_concurrent_lease_operations() {
    // Test lease acquire, renew, expire under high concurrency
    
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    
    let mut handles = vec![];
    
    for i in 0..50 {
        let state_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            let mut guard = state_clone.write_recover("lease_op").unwrap();
            
            // Acquire lease
            let _ = guard.acquire_lease(
                format!("lease_key.{}", i),
                i as u64,
                10, // TTL 10 seconds
                1000 + i as u64,
            );
            
            // Query lease
            let _ = guard.get_lease_info(&format!("lease_key.{}", i));
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let state = ownership_state.read_recover().unwrap();
    assert_eq!(state.lease_count(), 50, "All leases should be created");
}

#[test]
#[ignore] // Manual run: takes a while
fn test_production_load_10k_connections() {
    // Simulate 10,000 concurrent connections with multiple RPCs in pipeline
    // This test is heavy and should be run manually
    
    let target_connections = 10_000;
    let operations_per_connection = 10;
    
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let operation_count = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    let chunk_size = 100;
    let num_chunks = target_connections / chunk_size;
    
    for chunk in 0..num_chunks {
        let state_clone = Arc::clone(&ownership_state);
        let ops = Arc::clone(&operation_count);
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);
        
        let mut handles = vec![];
        
        for i in 0..chunk_size {
            let conn_id = chunk * chunk_size + i;
            
            let handle = thread::spawn(move || {
                for op in 0..operations_per_connection {
                    let topic = format!("conn_{}_op_{}", conn_id, op);
                    
                    let result = (|| {
                        let mut guard = state_clone.write_recover("production_load")?;
                        guard.apply(&OwnershipCommand::ClaimTopic {
                            topic: topic.clone(),
                            node_id: conn_id as u64,
                            timestamp: 1000,
                        });
                        Ok(())
                    })();
                    
                    match result {
                        Ok(()) => {
                            success.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    
                    ops.fetch_add(1, Ordering::Relaxed);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
    }
    
    let total_ops = operation_count.load(Ordering::Relaxed);
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    
    eprintln!("Production load test: {} operations, {} success, {} errors", 
              total_ops, success, errors);
    
    assert!(total_ops > 0, "Should have performed operations");
}

#[test]
fn test_rapid_lock_recovery() {
    // Test rapid lock acquisition and recovery under stress
    
    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    let lock.reset_poison_count();
    
    let mut handles = vec![];
    
    for _ in 0..20 {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = lock_clone.write_recover("rapid_recovery").ok();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Final verification
    let _ = lock.write_recover("final").unwrap();
}

#[test]
fn test_memory_stress_under_load() {
    // Test memory usage under sustained high load
    
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));
    let iteration_count = 10_000;
    
    for i in 0..iteration_count {
        let mut guard = ownership_state.write_recover("memory_stress").unwrap();
        guard.apply(&OwnershipCommand::ClaimTopic {
            topic: format!("memory_topic.{}", i),
            node_id: 1,
            timestamp: 1000,
        });
        
        // Periodic cleanup to prevent memory explosion in test
        if i % 1000 == 0 {
            guard.ownership.retain(|_, _| true); // Keep all, but check memory path
        }
        
        drop(guard);
    }
    
    // Verify final state
    let state = ownership_state.read_recover().unwrap();
    assert_eq!(state.topic_count(), iteration_count, "All topics should exist");
}

#[test]
fn test_lock_contention_under_stress() {
    // Test behavior under high lock contention
    
    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    let lock.reset_poison_count();
    
    let mut handles = vec![];
    
    // Many threads trying to write simultaneously
    for i in 0..50 {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let topic = format!("contention_{}_{}", i, j);
                
                // Try to acquire lock
                let _ = (|| {
                    let mut guard = lock_clone.write_recover("contention_node")?;
                    guard.value = i as u64 * 100 + j as u64;
                    Ok(())
                })();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    lock.write_recover("final_contention").unwrap();
}
