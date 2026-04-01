//! Performance benchmarks for critical fixes
//!
//! Benchmarks for lock recovery overhead, memory ordering, and validation.

use criterion::{criterion_group, criterion_main, Criterion};
use std::sync::Arc;

use autoqueues::cluster::lock_utils::{RecoverableLock, SafeLock, ValidationError};
use autoqueues::cluster::ownership_machine::OwnershipState;

/// Test state for benchmarks
#[derive(Debug, Clone)]
struct BenchState {
    value: u64,
}

impl Default for BenchState {
    fn default() -> Self {
        Self { value: 0 }
    }
}

impl Validate for BenchState {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

fn bench_lock_recovery_happy_path(c: &mut Criterion) {
    c.bench_function("lock_recovery_happy_path", |b| {
        let lock = RecoverableLock::new(BenchState::default());
        
        b.iter(|| {
            let _guard = lock.write_recover("bench_happy_path").unwrap();
        });
    });
}

fn bench_lock_recovery_poisoned(c: &mut Criterion) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    
    let lock = Arc::new(RecoverableLock::new(BenchState::default()));
    
    // Pre-cause a poison
    let lock_clone = Arc::clone(&lock);
    let handle = thread::spawn(move || {
        let _guard = lock_clone.write_recover("pre_poison").unwrap();
        panic!("Pre-poison");
    });
    
    handle.join().unwrap_err();
    
    c.bench_function("lock_recovery_poisoned", |b| {
        let lock_clone = Arc::clone(&lock);
        
        b.iter(|| {
            let _ = lock_clone.write_recover("bench_poisoned");
        });
    });
}

fn bench_lock_read_vs_write(c: &mut Criterion) {
    let lock = RecoverableLock::new(BenchState::default());
    
    c.bench_function("lock_read_uncontended", |b| {
        b.iter(|| {
            let _guard = lock.read_recover().unwrap();
        });
    });
    
    c.bench_function("lock_write_uncontended", |b| {
        b.iter(|| {
            let _guard = lock.write_recover("bench_write").unwrap();
        });
    });
}

fn bench_ownership_state_operation(c: &mut Criterion) {
    let ownership_state = RecoverableLock::new(OwnershipState::new());
    
    c.bench_function("ownership_state_apply_command", |b| {
        b.iter(|| {
            let mut guard = ownership_state.write_recover("bench_command").unwrap();
            guard.apply(&autoqueues::cluster::ownership_machine::OwnershipCommand::ClaimTopic {
                topic: "bench_topic".to_string(),
                node_id: 1,
                timestamp: 1000,
            });
        });
    });
}

fn bench_lock_recovery_overhead(c: &mut Criterion) {
    let lock = RecoverableLock::new(BenchState::default());
    
    c.bench_function("lock_recovery_overhead", |b| {
        b.iter(|| {
            let _guard = lock.write_recover("bench_overhead").unwrap();
            // Small workload
            let val = 42u64;
            drop(val);
        });
    });
}

fn bench_memory_ordering_comparison(c: &mut Criterion) {
    // Compare different memory orderings
    
    let atomic = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    c.bench_function("atomic_ordering_relaxed", |b| {
        let atomic = Arc::clone(&atomic);
        b.iter(|| {
            atomic.store(1, std::sync::atomic::Ordering::Relaxed);
        });
    });
    
    c.bench_function("atomic_ordering_acquire", |b| {
        let atomic = Arc::clone(&atomic);
        b.iter(|| {
            atomic.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        });
    });
    
    c.bench_function("atomic_ordering_acq_rel", |b| {
        let atomic = Arc::clone(&atomic);
        b.iter(|| {
            atomic.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        });
    });
}

fn bench_poison_validation_overhead(c: &mut Criterion) {
    let lock = Arc::new(RecoverableLock::new(BenchState::default()));
    
    // Pre-poison
    let lock_clone = Arc::clone(&lock);
    let handle = thread::spawn(move || {
        let _guard = lock_clone.write_recover("pre_poison").unwrap();
        panic!("Pre-poison");
    });
    
    handle.join().unwrap_err();
    
    c.bench_function("poison_validation_recovery", |b| {
        let lock_clone = Arc::clone(&lock);
        
        b.iter(|| {
            let result = lock_clone.write_recover("bench_validation");
            let _ = result;
        });
    });
}

fn bench_concurrent_lock_operations(c: &mut Criterion) {
    let lock = Arc::new(RecoverableLock::new(BenchState::default()));
    
    c.bench_function("concurrent_lock_read", |b| {
        b.iter(|| {
            let lock_clone = Arc::clone(&lock);
            let _guard = lock_clone.read_recover().unwrap();
            drop(_guard);
        });
    });
}

fn bench_state_serialization_overhead(c: &mut Criterion) {
    let ownership_state = RecoverableLock::new(OwnershipState::new());
    
    // Populate state first
    {
        let mut guard = ownership_state.write_recover("setup").unwrap();
        for i in 0..100 {
            guard.apply(&autoqueues::cluster::ownership_machine::OwnershipCommand::ClaimTopic {
                topic: format!("topic_{}", i),
                node_id: i as u64,
                timestamp: 1000,
            });
        }
    }
    
    c.bench_function("state_serialization_size", |b| {
        let guard = ownership_state.read_recover().unwrap();
        
        // Just measure the size of the state structure
        b.iter(|| {
            let _ = std::mem::size_of::<OwnershipState>();
        });
    });
}

fn bench_ownership_query_operations(c: &mut Criterion) {
    let ownership_state = RecoverableLock::new(OwnershipState::new());
    
    // Populate
    {
        let mut guard = ownership_state.write_recover("setup").unwrap();
        for i in 0..50 {
            guard.apply(&autoqueues::cluster::ownership_machine::OwnershipCommand::ClaimTopic {
                topic: format!("query_topic_{}", i),
                node_id: i as u64,
                timestamp: 1000,
            });
        }
    }
    
    c.bench_function("ownership_query_get_owner", |b| {
        let guard = ownership_state.read_recover().unwrap();
        
        b.iter(|| {
            let _owner = guard.get_owner("query_topic_0");
        });
    });
    
    c.bench_function("ownership_query_list_topics", |b| {
        let guard = ownership_state.read_recover().unwrap();
        
        b.iter(|| {
            let _topics = guard.get_node_topics(0);
        });
    });
}

// Create criterion group
criterion_group!(
    critical_fixes_benches,
    bench_lock_recovery_happy_path,
    bench_lock_recovery_poisoned,
    bench_lock_read_vs_write,
    bench_ownership_state_operation,
    bench_lock_recovery_overhead,
    bench_memory_ordering_comparison,
    bench_poison_validation_overhead,
    bench_concurrent_lock_operations,
    bench_state_serialization_overhead,
    bench_ownership_query_operations,
);

// Create criterion main
criterion_main!(critical_fixes_benches);
