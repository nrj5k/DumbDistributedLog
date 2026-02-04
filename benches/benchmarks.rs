//! Benchmark tests for AutoQueues performance
//!
//! Run with: cargo bench --bench benchmarks

use autoqueues::{
    expression::{compile_expression, evaluate_expression},
    queue::{
        spmc_lockfree_queue::SPMCLockFreeQueue,
        simple_queue::SimpleQueue,
        lockfree::LockFreeQueue,
    },
    traits::queue::QueueTrait,
};
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

/// Benchmark: Expression compilation and evaluation
fn bench_expression_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("expression_evaluation");
    
    // Complex expression with functions
    let complex_expr = compile_expression(
        "(x.sqrt() + y.pow(1.5)) / 2.0 + z.max(w)"
    ).unwrap();

    group.bench_function("compile_simple", |b| {
        b.iter(|| {
            compile_expression("a + b").unwrap()
        })
    });

    group.bench_function("eval_simple", |b| {
        let vars: HashMap<String, f64> = vec![
            ("a", 50.0),
            ("b", 30.0),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        
        b.iter(|| {
            evaluate_expression("a + b", &vars).unwrap()
        })
    });

    group.bench_function("eval_complex", |b| {
        let vars: HashMap<String, f64> = vec![
            ("x", 50.0),
            ("y", 60.0),
            ("z", 70.0),
            ("w", 80.0),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        
        b.iter(|| {
            complex_expr.evaluate(&vars).unwrap()
        })
    });

    group.bench_function("eval_10_vars", |b| {
        let mut vars = HashMap::new();
        for i in 0..10 {
            vars.insert(format!("v{}", i), i as f64 * 10.0);
        }
        
        b.iter(|| {
            evaluate_expression(
                "(v0 + v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9) / 10.0",
                &vars
            ).unwrap()
        })
    });

    group.finish();
}

/// Benchmark: SPMC Lock-free Queue performance
fn bench_spmc_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("spmc_lockfree_queue");
    group.sample_size(1000);
    
    // Test with different queue capacities
    for capacity in [256, 1024, 4096] {
        group.bench_function(&format!("push_{}_items", capacity), |b| {
            b.iter_batched(
                || SPMCLockFreeQueue::<f64, 4096>::new(),
                |mut queue| {
                    let mut pushed = 0;
                    for i in 0..capacity {
                        if queue.push(i as f64).is_ok() {
                            pushed += 1;
                        }
                    }
                    pushed
                },
                criterion::BatchSize::PerIteration,
            );
        });
    }

    group.bench_function("push_pop_1000", |b| {
        b.iter_batched(
            || SPMCLockFreeQueue::<f64, 4096>::new(),
            |mut queue| {
                for i in 0..1000 {
                    let _ = queue.push(i as f64);
                }
                let consumer = queue.consumer();
                let mut sum = 0.0;
                for _ in 0..1000 {
                    if let Some((_, val)) = consumer.pop() {
                        sum += val;
                    }
                }
                sum
            },
            criterion::BatchSize::PerIteration,
        );
    });

    group.finish();
}

/// Benchmark: Simple Queue (Ring Buffer) performance
fn bench_simple_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_queue");
    group.sample_size(1000);

    group.bench_function("push_1000", |b| {
        b.iter(|| {
            let mut queue = SimpleQueue::<f64>::new();
            let mut pushed = 0;
            for i in 0..1000 {
                if QueueTrait::publish(&mut queue, i as f64).is_ok() {
                    pushed += 1;
                }
            }
            pushed
        });
    });

    group.bench_function("push_pop_1000", |b| {
        b.iter(|| {
            let mut queue = SimpleQueue::<f64>::new();
            for i in 0..1000 {
                let _ = QueueTrait::publish(&mut queue, i as f64);
            }
            let mut sum = 0.0f64;
            for _ in 0..1000 {
                if let Some((_, val)) = QueueTrait::get_latest(&queue) {
                    sum += val;
                }
            }
            sum
        });
    });

    group.finish();
}

/// Benchmark: Lock-free Queue performance
fn bench_lockfree_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_queue");
    group.sample_size(1000);

    group.bench_function("push_1000", |b| {
        b.iter(|| {
            let mut queue = LockFreeQueue::<f64, 4096>::new();
            let mut pushed = 0;
            for i in 0..1000 {
                if QueueTrait::publish(&mut queue, i as f64).is_ok() {
                    pushed += 1;
                }
            }
            pushed
        });
    });

    group.bench_function("push_pop_1000", |b| {
        b.iter(|| {
            let mut queue = LockFreeQueue::<f64, 4096>::new();
            for i in 0..1000 {
                let _ = QueueTrait::publish(&mut queue, i as f64);
            }
            let mut sum = 0.0f64;
            for _ in 0..1000 {
                if let Some((_, val)) = QueueTrait::get_latest(&queue) {
                    sum += val;
                }
            }
            sum
        });
    });

    group.finish();
}

/// Benchmark: Queue creation overhead
fn bench_queue_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_creation");

    group.bench_function("spmc_new", |b| {
        b.iter(|| {
            SPMCLockFreeQueue::<f64, 1024>::new();
        })
    });

    group.bench_function("simple_new", |b| {
        b.iter(|| {
            SimpleQueue::<f64>::new();
        })
    });

    group.bench_function("lockfree_new", |b| {
        b.iter(|| {
            LockFreeQueue::<f64, 1024>::new();
        })
    });

    group.finish();
}

/// Throughput test: Maximum operations per second
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.sample_size(100);

    group.bench_function("spmc_100k_ops", |b| {
        b.iter(|| {
            let mut queue = SPMCLockFreeQueue::<u64, 131072>::new();
            let start = std::time::Instant::now();
            let mut count = 0;
            
            // Single thread push
            for i in 0..100_000 {
                let _ = queue.push(i);
            }
            
            // Single thread pop with consumer
            let consumer = queue.consumer();
            while let Some((_, _)) = consumer.pop() {
                count += 1;
            }
            
            let elapsed = start.elapsed();
            let micros = elapsed.as_micros();
            let ops_per_sec = if micros > 0 {
                (count as u128 * 1_000_000) / micros
            } else {
                count as u128
            };
            ops_per_sec
        });
    });

    group.finish();
}

/// Latency test: Individual operation latency
fn bench_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency");
    group.sample_size(10000);

    group.bench_function("spmc_single_push", |b| {
        b.iter(|| {
            let mut queue = SPMCLockFreeQueue::<f64, 4096>::new();
            let _ = queue.push(42.0);
        });
    });

    group.bench_function("spmc_single_pop", |b| {
        b.iter(|| {
            let queue = SPMCLockFreeQueue::<f64, 4096>::new();
            let consumer = queue.consumer();
            let _ = consumer.pop();
        });
    });
    
    group.bench_function("expression_eval", |b| {
        let vars: HashMap<String, f64> = vec![
            ("a", 10.0),
            ("b", 20.0),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        
        b.iter(|| {
            evaluate_expression("a + b * 2 - 5", &vars).unwrap()
        });
    });

    group.finish();
}

/// Compare different queue implementations
fn bench_queue_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_comparison");
    group.sample_size(1000);
    
    const OPS: usize = 10_000;

    group.bench_function("spmc_10k_ops", |b| {
        b.iter(|| {
            let mut queue = SPMCLockFreeQueue::<usize, 16384>::new();
            for i in 0..OPS {
                let _ = queue.push(i);
            }
            let consumer = queue.consumer();
            let mut sum = 0usize;
            for _ in 0..OPS {
                if let Some((_, v)) = consumer.pop() {
                    sum += v;
                }
            }
            sum
        });
    });

    group.bench_function("simple_10k_ops", |b| {
        b.iter(|| {
            let mut queue = SimpleQueue::<usize>::new();
            for i in 0..OPS {
                let _ = QueueTrait::publish(&mut queue, i);
            }
            let mut sum = 0usize;
            for _ in 0..OPS {
                if let Some((_, v)) = QueueTrait::get_latest(&queue) {
                    sum += v;
                }
            }
            sum
        });
    });

    group.bench_function("lockfree_10k_ops", |b| {
        b.iter(|| {
            let mut queue = LockFreeQueue::<usize, 16384>::new();
            for i in 0..OPS {
                let _ = QueueTrait::publish(&mut queue, i);
            }
            let mut sum = 0usize;
            for _ in 0..OPS {
                if let Some((_, v)) = QueueTrait::get_latest(&queue) {
                    sum += v;
                }
            }
            sum
        });
    });

    group.finish();
}

/// Memory footprint comparison
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");
    group.sample_size(100);

    use std::mem::size_of;
    
    group.bench_function("sizeof_spmc_queue", |b| {
        b.iter(|| {
            let _queue = SPMCLockFreeQueue::<f64, 1024>::new();
            size_of::<SPMCLockFreeQueue<f64, 1024>>()
        });
    });

    group.bench_function("sizeof_simple_queue", |b| {
        b.iter(|| {
            let _queue = SimpleQueue::<f64>::new();
            size_of::<SimpleQueue<f64>>()
        });
    });

    group.bench_function("sizeof_lockfree_queue", |b| {
        b.iter(|| {
            let _queue = LockFreeQueue::<f64, 1024>::new();
            size_of::<LockFreeQueue<f64, 1024>>()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_expression_evaluation,
    bench_spmc_queue,
    bench_simple_queue,
    bench_lockfree_queue,
    bench_queue_creation,
    bench_throughput,
    bench_latency,
    bench_queue_comparison,
    bench_memory_footprint,
);

criterion_main!(benches);