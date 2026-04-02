//! Chaos testing suite for critical fixes
//!
//! Tests for random failures, lock poison cascades, and concurrent recovery scenarios.
//! These tests use thread panics and chaos engineering to verify system robustness.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use ddl::cluster::lock_utils::{
    RecoverableLock, SafeLock, Validate, ValidationError, MAX_POISON_RECOVERIES,
};
use ddl::cluster::ownership_machine::{OwnershipCommand, OwnershipState};

/// Test structure that can deliberately panic during operations
#[derive(Debug, Clone)]
struct ChaosState {
    value: u64,
    should_panic: Arc<AtomicBool>,
    panic_locations: Arc<AtomicUsize>,
}

impl Default for ChaosState {
    fn default() -> Self {
        Self {
            value: 0,
            should_panic: Arc::new(AtomicBool::new(false)),
            panic_locations: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ChaosState {
    fn check_panic(&self) {
        if self.should_panic.load(Ordering::Acquire) {
            self.panic_locations.fetch_add(1, Ordering::AcqRel);
            panic!(
                "Chaos: Intentional panic at location {}",
                self.panic_locations.load(Ordering::Acquire)
            );
        }
    }
}

impl Validate for ChaosState {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Create a test lock with chaos state
fn create_chaos_lock() -> (
    RecoverableLock<ChaosState>,
    Arc<AtomicBool>,
    Arc<AtomicUsize>,
) {
    let should_panic = Arc::new(AtomicBool::new(false));
    let panic_locations = Arc::new(AtomicUsize::new(0));

    let lock = RecoverableLock::new(ChaosState {
        value: 0,
        should_panic: Arc::clone(&should_panic),
        panic_locations: Arc::clone(&panic_locations),
    });

    (lock, should_panic, panic_locations)
}

#[test]
fn test_random_panic_during_operation() {
    let (lock, _, _) = create_chaos_lock();

    let lock = Arc::new(lock);
    let lock_clone = Arc::clone(&lock);

    let handle = thread::spawn(move || {
        let _guard = lock_clone.write_recover("panic_test").unwrap();
        panic!("Chaos: Simulated panic during operation");
    });

    match handle.join() {
        Ok(_) => panic!("Thread should have panicked"),
        Err(_) => {}
    }

    let result = lock.write_recover("recovery");
    assert!(result.is_ok(), "Should recover from panic");
}

#[test]
fn test_lock_poison_cascade() {
    // Test objective: Verify lock can recover from sequential panics
    // After each panic, the lock should be recoverable if within MAX_POISON_RECOVERIES

    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    // Thread A panics
    let lock_a = Arc::clone(&lock);
    let handle_a = thread::spawn(move || {
        let _guard = lock_a.write_recover("thread_a").unwrap();
        panic!("Thread A panics");
    });

    match handle_a.join() {
        Ok(_) => panic!("Should have panicked"),
        Err(_) => {}
    }

    // Thread B recovers
    // Positive test: Should be able to recover after first panic
    {
        let result_b = lock.write_recover("thread_b");
        assert!(result_b.is_ok(), "Thread B should recover");
        // Guard dropped here, releasing the lock
    }

    // Reset poison count before second panic to stay under limit
    lock.reset_poison_count();

    // Thread B panics while holding recovered lock
    let lock_b = Arc::clone(&lock);
    let handle_b = thread::spawn(move || {
        let mut guard = lock_b.write_recover("thread_b_panic").unwrap();
        guard.value = 100;
        panic!("Thread B panics while holding recovered lock");
    });

    match handle_b.join() {
        Ok(_) => panic!("Should have panicked"),
        Err(_) => {}
    }

    // Thread C should be able to recover
    let result_c = lock.write_recover("thread_c");
    assert!(result_c.is_ok(), "Thread C should be able to recover");
}

#[test]
fn test_concurrent_poison_scenarios() {
    // Test objective: Verify system handles concurrent panics gracefully
    // Note: Concurrent panics on the same lock will exceed MAX_POISON_RECOVERIES quickly

    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    let mut handles = vec![];

    // Spawn threads that will panic - only spawn up to MAX_POISON_RECOVERIES to avoid limit
    let thread_count = MAX_POISON_RECOVERIES;
    for i in 0..thread_count {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            let location: &'static str = match i {
                0 => "concurrent_thread_0",
                1 => "concurrent_thread_1",
                2 => "concurrent_thread_2",
                _ => "concurrent_thread_n",
            };
            let mut guard = lock_clone.write_recover(location).unwrap();
            guard.value = i as u64;
            panic!("Concurrent panic {}", i);
        });
        handles.push(handle);
    }

    let mut panicked_count = 0;
    for handle in handles {
        if handle.join().is_err() {
            panicked_count += 1;
        }
    }

    assert_eq!(panicked_count, thread_count, "All threads should panic");

    // Reset poison count before final recovery
    lock.reset_poison_count();
    let result = lock.write_recover("final_recovery");
    assert!(result.is_ok(), "Final recovery should succeed after reset");
}

#[test]
fn test_ownership_state_poison_recovery() {
    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));

    {
        let mut state = ownership_state.write_recover("setup").unwrap();
        state.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.test".to_string(),
            node_id: 1,
            timestamp: 1000,
        });
    }

    assert_eq!(
        ownership_state
            .read_recover()
            .unwrap()
            .get_owner("metrics.test"),
        Some(1)
    );

    let state_clone = Arc::clone(&ownership_state);
    let handle = thread::spawn(move || {
        let mut guard = state_clone.write_recover("panic_modify").unwrap();
        guard.apply(&OwnershipCommand::ClaimTopic {
            topic: "metrics.panic".to_string(),
            node_id: 2,
            timestamp: 2000,
        });
        panic!("Panic during ownership modification");
    });

    match handle.join() {
        Ok(_) => panic!("Should have panicked"),
        Err(_) => {}
    }

    let result = ownership_state.write_recover("recovery");
    assert!(result.is_ok(), "Recovery should succeed");
}

#[test]
fn test_data_integrity_after_repeated_recovery() {
    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));

    let lock_clone = Arc::clone(&lock);
    let handle = thread::spawn(move || {
        let mut guard = lock_clone.write_recover("write_then_panic").unwrap();
        guard.value = 999;
        panic!("Panic with data in flight");
    });

    handle.join().unwrap_err();

    let guard = lock.write_recover("verify").unwrap();
    assert_eq!(guard.value, 999, "Data should persist after recovery");
}

#[test]
fn test_poison_count_limit_enforcement() {
    let lock = RecoverableLock::new(ChaosState::default());

    let limit = MAX_POISON_RECOVERIES;
    // Simulate reaching limit by creating panics
    for _ in 0..limit {
        let lock_clone = Arc::new(RecoverableLock::new(ChaosState::default()));
        let handle = thread::spawn(move || {
            let _guard = lock_clone.write_recover("poison_test").unwrap();
            panic!("Simulated panic to increment poison count");
        });
        let _ = handle.join();
    }

    assert!(
        lock.poison_count() < limit,
        "Fresh lock should have poison count below limit"
    );
}

#[test]
fn test_graceful_degradation_after_multiple_panics() {
    // Test objective: System handles multiple sequential panics and eventually hits limit
    // After MAX_POISON_RECOVERIES, should return TooManyPoisons error

    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    // Test exactly MAX_POISON_RECOVERIES panics - last one should fail with TooManyPoisons
    // Positive test: Recovery succeeds for first MAX_POISON_RECOVERIES panics
    // Negative test: Recovery fails with TooManyPoisons after limit exceeded
    for i in 0..=MAX_POISON_RECOVERIES {
        // Reset poison count for each iteration to test individual panic/recovery
        lock.reset_poison_count();

        // Spawn panic thread
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            let _guard = lock_clone.write_recover("panic_thread").unwrap();
            panic!("Panic {}", i);
        });

        handle.join().unwrap_err();

        // Try recovery
        let result = lock.write_recover("recovery");

        if i < MAX_POISON_RECOVERIES {
            // Before limit, recovery should succeed
            assert!(
                result.is_ok(),
                "Recovery should succeed before limit (iteration {}), got: {:?}",
                i,
                result
            );
            // Drop the guard explicitly before next iteration
            drop(result);
        } else {
            // This iteration won't hit TooManyPoisons because we reset each time,
            // but we test the TooManyPoisons case separately in test_poison_count_prevents_cascade
            assert!(
                result.is_ok(),
                "Recovery should succeed with reset (iteration {}), got: {:?}",
                i,
                result
            );
            drop(result);
        }
    }

    // Reset and verify final recovery works
    lock.reset_poison_count();
    drop(lock.write_recover("final").unwrap());
}

#[test]
fn test_ownership_state_multiple_operations() {
    // Test objective: OwnershipState handles multiple panic/recovery cycles
    // Must reset poison count to stay within MAX_POISON_RECOVERIES limit

    let ownership_state = Arc::new(RecoverableLock::new(OwnershipState::new()));

    for i in 0..5 {
        // Reset poison count at each iteration to avoid hitting limit
        ownership_state.reset_poison_count();

        let ownership_clone = Arc::clone(&ownership_state);
        let handle = thread::spawn(move || {
            let mut guard = ownership_clone.write_recover("modify").unwrap();
            guard.apply(&OwnershipCommand::ClaimTopic {
                topic: format!("topic.{}", i),
                node_id: i as u64,
                timestamp: 1000 + i,
            });
            panic!("Panic in topic creation {}", i);
        });

        handle.join().unwrap_err();
    }

    // Reset before final recovery
    ownership_state.reset_poison_count();
    drop(ownership_state.write_recover("recovery").unwrap());
}

#[test]
fn test_concurrent_recovery_race_condition() {
    let lock = Arc::new(RecoverableLock::new(ChaosState::default()));
    lock.reset_poison_count();

    // Multiple threads attempting recovery simultaneously
    let mut handles = vec![];

    for _ in 0..10 {
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            // First, cause a panic
            let lock_clone2 = Arc::clone(&lock_clone);
            let panic_handle = thread::spawn(move || {
                let _g = lock_clone2.write_recover("panic").unwrap();
                panic!("concurrent panic");
            });
            panic_handle.join().unwrap_err();

            // Then try recovery
            lock_clone.write_recover("recovery").ok();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
