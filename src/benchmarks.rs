//! Comprehensive Benchmark Tests for AutoQueues
//!
//! Run with:
//!   cargo test --release --lib -- benchmarks -- --nocapture

#[cfg(test)]
mod benchmarks {

    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    use crate::queue::spmc_lockfree_queue::SPMCLockFreeQueue;

    // Only run expensive benchmarks in release mode
    macro_rules! release_only_test {
        ($name:ident, $body:block) => {
            #[test]
            fn $name() {
                if !cfg!(debug_assertions) {
                    $body
                } else {
                    println!(
                        "Skipping {} in debug mode (would overflow stack)",
                        stringify!($name)
                    );
                }
            }
        };
    }

    // Target: < 5 mins total runtime
    const RUNS: usize = 30;
    const WARMUP: usize = 3;

    /// Run benchmarks and collect statistics (avg, p95, min, max)
    fn benchmark_with_stats<F>(name: &str, iterations: usize, f: F)
    where
        F: Fn() -> u128 + Copy,
    {
        // Warmup
        for _ in 0..WARMUP {
            f();
        }

        // Collect samples
        let mut samples: Vec<u128> = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            samples.push(f());
        }

        // Calculate stats
        let total: u128 = samples.iter().sum();
        let avg_ns = total / iterations as u128;
        let avg_us = avg_ns as f64 / 1000.0;

        let min_ns = *samples.iter().min().unwrap();
        let max_ns = *samples.iter().max().unwrap();

        // Calculate p95
        let mut sorted = samples;
        sorted.sort();
        let p95_idx = (iterations * 95) / 100;
        let p95_ns = sorted[p95_idx];

        println!(
            "{:28} | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.0}-{:>8.0} ns",
            name,
            avg_us,
            p95_ns as f64 / 1000.0,
            max_ns as f64 / 1000.0,
            min_ns as f64 / 1000.0,
            min_ns,
            max_ns
        );
    }

    // ============ SEQUENTIAL QUEUE BENCHMARKS ============

    #[test]
    fn bench_queue_creation() {
        println!("\n=== QUEUE CREATION BENCHMARKS ===");
        println!(
            "{:28} | {:>8} | {:>8} | {:>8} | {:>8} | {:>17}",
            "Benchmark", "Avg us", "p95 us", "Max us", "Min us", "Range ns"
        );
        println!("{}", "-".repeat(78));

        benchmark_with_stats("spmc_queue_creation", RUNS, || {
            let start = Instant::now();
            let _ = SPMCLockFreeQueue::<f64, 1024>::new();
            start.elapsed().as_nanos()
        });
    }

    #[test]
    fn bench_queue_push() {
        println!("\n=== QUEUE PUSH BENCHMARKS (1,000 items) ===");
        println!(
            "{:28} | {:>8} | {:>8} | {:>8} | {:>8} | {:>17}",
            "Benchmark", "Avg us", "p95 us", "Max us", "Min us", "Range ns"
        );
        println!("{}", "-".repeat(78));

        benchmark_with_stats("spmc_push_1000", RUNS, || {
            let start = Instant::now();
            let mut queue = SPMCLockFreeQueue::<f64, 4096>::new();
            for i in 0..1000 {
                let _ = queue.push(i as f64);
            }
            start.elapsed().as_nanos()
        });
    }

    // ============ CORE: CONCURRENT PULL BENCHMARKS ============
    // This is THE benchmark that shows SPMC advantage:
    // Multiple consumers pull from the same queue concurrently

    release_only_test!(bench_spmc_concurrent_pull, {
        println!("\n=== CORE: SPMC CONCURRENT PULL (10,000 items) ===");
        println!(
            "{:28} | {:>8} | {:>8} | {:>8} | {:>8} | {:>17}",
            "Benchmark", "Avg us", "p95 us", "Max us", "Min us", "Range ns"
        );
        println!("{}", "-".repeat(78));

        // Reduce items and consumers to prevent stack overflow in debug builds
        const ITEMS: usize = 10_000;
        const CONSUMER_COUNTS: &[usize] = &[1, 2]; // Removed 4 consumers to reduce stress
        const ITERATIONS: usize = 2; // Further reduced iterations from RUNS/2 = 15 to prevent stack overflow

        for &num_consumers in CONSUMER_COUNTS {
            let name = format!("spmc_concurrent_{}pull", num_consumers);
            let _items_per_consumer = ITEMS / num_consumers;

            // Collect timing samples manually instead of using benchmark_with_stats
            // to reduce stack pressure in debug builds with multiple thread spawns
            let mut samples: Vec<u128> = Vec::with_capacity(ITERATIONS);

            // Warmup run - reduce capacity to match reduced ITEM count
            run_concurrent_pull_test(ITEMS, num_consumers);

            // Collect samples
            for _ in 0..ITERATIONS {
                let start = Instant::now();
                run_concurrent_pull_test(ITEMS, num_consumers);
                samples.push(start.elapsed().as_nanos());
            }

            // Calculate stats manually
            let total: u128 = samples.iter().sum();
            let avg_ns = total / ITERATIONS as u128;
            let avg_us = avg_ns as f64 / 1000.0;

            let min_ns = *samples.iter().min().unwrap();
            let max_ns = *samples.iter().max().unwrap();

            // Calculate p95
            let mut sorted = samples;
            sorted.sort();
            let p95_idx = (ITERATIONS * 95) / 100;
            let p95_ns = sorted[p95_idx];

            println!(
                "{:28} | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.0}-{:>8.0} ns",
                name,
                avg_us,
                p95_ns as f64 / 1000.0,
                max_ns as f64 / 1000.0,
                min_ns as f64 / 1000.0,
                min_ns,
                max_ns
            );
        }
    });

    release_only_test!(bench_spmc_concurrent_pull_throughput, {
        println!("\n=== CORE: SPMC CONCURRENT PULL THROUGHPUT (15k items) ===");
        println!(
            "{:28} | {:>10} | {:>10} | {:>10}",
            "Benchmark", "Avg us", "Items/sec", "Speedup"
        );
        println!("{}", "-".repeat(62));

        const ITEMS: usize = 15_000;
        const CONSUMER_COUNTS: &[usize] = &[1, 2, 4];

        // First, run baseline (1 consumer)
        let items_per_consumer = ITEMS;
        let start = Instant::now();

        let mut queue = SPMCLockFreeQueue::<u64, 32768>::new();
        for i in 0..ITEMS {
            let _ = queue.push(i as u64);
        }
        let queue_arc = Arc::new(queue);

        let mut handles = vec![];
        for _ in 0..1 {
            let queue = queue_arc.clone();
            handles.push(thread::spawn(move || {
                let consumer = queue.consumer();
                let mut count = 0;
                while count < items_per_consumer {
                    if let Some((_, val)) = consumer.pop() {
                        // Simulate real work: some computation
                        let _ = val * 2 + 1;
                        count += 1;
                    }
                }
                count
            }));
        }

        for h in handles {
            let _ = h.join().unwrap();
        }

        let baseline_us = start.elapsed().as_micros() as f64;
        let baseline_items_per_sec = (ITEMS as f64 / baseline_us) * 1_000_000.0;
        println!(
            "{:28} | {:>10.2} | {:>10.0} | {:>10.1}x",
            "spmc_1pull (work)", baseline_us, baseline_items_per_sec, 1.0
        );

        // Now test with multiple consumers
        for &num_consumers in &CONSUMER_COUNTS[1..] {
            let items_per_consumer = ITEMS / num_consumers;
            let start = Instant::now();

            let mut queue = SPMCLockFreeQueue::<u64, 32768>::new();
            for i in 0..ITEMS {
                let _ = queue.push(i as u64);
            }
            let queue_arc = Arc::new(queue);

            let mut handles = vec![];

            for _ in 0..num_consumers {
                let queue = queue_arc.clone();
                handles.push(thread::spawn(move || {
                    let consumer = queue.consumer();
                    let mut count = 0;
                    while count < items_per_consumer {
                        if let Some((_, val)) = consumer.pop() {
                            // Simulate real work
                            let _ = val * 2 + 1;
                            count += 1;
                        }
                    }
                    count
                }));
            }

            for h in handles {
                let _ = h.join().unwrap();
            }

            let elapsed_us = start.elapsed().as_micros() as f64;
            let items_per_sec = (ITEMS as f64 / elapsed_us) * 1_000_000.0;
            let speedup = baseline_us / elapsed_us;

            println!(
                "{:28} | {:>10.2} | {:>10.0} | {:>10.1}x",
                format!("spmc_{}pull (work)", num_consumers),
                elapsed_us,
                items_per_sec,
                speedup
            );
        }
    });

    /// Flexible scalability test
    /// Args: min_consumers, max_consumers, scale_factor, items, ops_per_item
    /// Example: min=2, max=20, factor=2 → runs: 2, 4, 8, 16, 20 (20 not in sequence, warns)
    release_only_test!(bench_spmc_flexible_scalability, {
        // Configurable parameters
        let min_consumers: usize = 2;
        let max_consumers: usize = 20;
        let scale_factor: usize = 2;
        const ITEMS: usize = 5_000;
        const OPS_PER_ITEM: usize = 100;

        println!(
            "\n=== SPMC FLEXIBLE SCALABILITY ({} items, {} ops/item) ===",
            ITEMS, OPS_PER_ITEM
        );
        println!(
            "Config: min={}, max={}, factor={}",
            min_consumers, max_consumers, scale_factor
        );
        println!(
            "{:28} | {:>10} | {:>10} | {:>10} | {:>10}",
            "Benchmark", "Avg us", "Items/sec", "Speedup", "Efficiency"
        );
        println!("{}", "-".repeat(72));

        // Build scale sequence: min, min*factor, min*factor^2, ... until >= max
        let mut scales: Vec<usize> = Vec::new();
        let mut current = min_consumers;
        while current < max_consumers {
            scales.push(current);
            current *= scale_factor;
        }
        scales.push(max_consumers); // Always include max

        // Check if max was in the sequence (for warning)
        let in_sequence = scales[..scales.len() - 1].contains(&max_consumers);
        if !in_sequence {
            let prev = scales[scales.len() - 2];
            let seq_str = if scales.len() > 2 {
                format!("{}, {}, ...", scales[0], prev)
            } else {
                format!("{}", scales[0])
            };
            println!(
                "⚠️  Warning: {} not in sequence ({}). Added extra run.",
                max_consumers, seq_str
            );
        }

        // Run baseline (1 consumer)
        let items_per_consumer = ITEMS;
        let start = Instant::now();

        let mut queue = SPMCLockFreeQueue::<u64, 16384>::new();
        for i in 0..ITEMS {
            let _ = queue.push(i as u64);
        }
        let queue_arc = Arc::new(queue);

        let mut handles = vec![];
        for _ in 0..1 {
            let queue = queue_arc.clone();
            handles.push(thread::spawn(move || {
                let consumer = queue.consumer();
                let mut count = 0;
                while count < items_per_consumer {
                    if let Some((_, val)) = consumer.pop() {
                        let mut result = val;
                        for _ in 0..OPS_PER_ITEM {
                            result = result.wrapping_mul(3).wrapping_add(1) % 1000000;
                        }
                        count += 1;
                    }
                }
                count
            }));
        }

        for h in handles {
            let _ = h.join().unwrap();
        }

        let baseline_us = start.elapsed().as_micros() as f64;
        let baseline_items_per_sec = (ITEMS as f64 / baseline_us) * 1_000.0;
        println!(
            "{:28} | {:>10.2} | {:>10.0} | {:>10.2}x | {:>10.2}x",
            "baseline_1consumer", baseline_us, baseline_items_per_sec, 1.0, 1.0
        );

        // Run at each scale
        for &num_consumers in &scales {
            let items_per_consumer = ITEMS / num_consumers;
            let start = Instant::now();

            let mut queue = SPMCLockFreeQueue::<u64, 16384>::new();
            for i in 0..ITEMS {
                let _ = queue.push(i as u64);
            }
            let queue_arc = Arc::new(queue);

            let mut handles = vec![];

            for _ in 0..num_consumers {
                let queue = queue_arc.clone();
                handles.push(thread::spawn(move || {
                    let consumer = queue.consumer();
                    let mut count = 0;
                    while count < items_per_consumer {
                        if let Some((_, val)) = consumer.pop() {
                            let mut result = val;
                            for _ in 0..OPS_PER_ITEM {
                                result = result.wrapping_mul(3).wrapping_add(1) % 1000000;
                            }
                            count += 1;
                        }
                    }
                    count
                }));
            }

            for h in handles {
                let _ = h.join().unwrap();
            }

            let elapsed_us = start.elapsed().as_micros() as f64;
            let items_per_sec = (ITEMS as f64 / elapsed_us) * 1_000.0;
            let speedup = baseline_us / elapsed_us;
            let efficiency = speedup / num_consumers as f64;

            // Highlight best result
            let marker = if speedup > 1.2 && num_consumers >= 4 {
                " ★"
            } else {
                ""
            };

            println!(
                "{:28} | {:>10.2} | {:>10.0} | {:>10.2}x | {:>10.2}x{}",
                format!("spmc_{}consumers", num_consumers),
                elapsed_us,
                items_per_sec,
                speedup,
                efficiency,
                marker
            );
        }
    });

    // ============ CONCURRENT DRAIN BENCHMARKS ============

    release_only_test!(bench_spmc_concurrent_drain, {
        println!("\n=== SPMC CONCURRENT DRAIN (full queue drain time) ===");
        println!(
            "{:28} | {:>8} | {:>8} | {:>8} | {:>8} | {:>17}",
            "Benchmark", "Avg us", "p95 us", "Max us", "Min us", "Range ns"
        );
        println!("{}", "-".repeat(78));

        // Reduce items and consumers to prevent stack overflow in debug builds
        const ITEMS: usize = 1_000; // Reduced from 10,000
        const CONSUMER_COUNTS: &[usize] = &[1, 2]; // Removed 4 consumers
        const ITERATIONS: usize = 2; // Further reduced iterations

        for &num_consumers in CONSUMER_COUNTS {
            let name = format!("spmc_drain_{}consumers", num_consumers);

            // Collect timing samples manually instead of using benchmark_with_stats
            // to reduce stack pressure in debug builds
            let mut samples: Vec<u128> = Vec::with_capacity(ITERATIONS);

            // Warmup run
            run_drain_test(ITEMS, num_consumers);

            // Collect samples
            for _ in 0..ITERATIONS {
                let start = Instant::now();
                run_drain_test(ITEMS, num_consumers);
                samples.push(start.elapsed().as_nanos());
            }

            // Calculate stats manually
            let total: u128 = samples.iter().sum();
            let avg_ns = total / ITERATIONS as u128;
            let avg_us = avg_ns as f64 / 1000.0;

            let min_ns = *samples.iter().min().unwrap();
            let max_ns = *samples.iter().max().unwrap();

            // Calculate p95
            let mut sorted = samples;
            sorted.sort();
            let p95_idx = (ITERATIONS * 95) / 100;
            let p95_ns = sorted[p95_idx];

            println!(
                "{:28} | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.2} us | {:>8.0}-{:>8.0} ns",
                name,
                avg_us,
                p95_ns as f64 / 1000.0,
                max_ns as f64 / 1000.0,
                min_ns as f64 / 1000.0,
                min_ns,
                max_ns
            );
        }
    });

    /// Helper function to run the actual concurrent pull test with proper item distribution
    fn run_concurrent_pull_test(items: usize, num_consumers: usize) {
        // Single producer fills queue (SPMC constraint)
        let mut queue = SPMCLockFreeQueue::<u64, 32768>::new(); // Match original capacity
        for i in 0..items {
            let _ = queue.push(i as u64);
        }
        let queue_arc = Arc::new(queue);

        // Use std::thread::scope to avoid stack buildup from spawning threads
        std::thread::scope(|s| {
            let mut handles = vec![];
            // Calculate items per consumer to prevent infinite loops
            let items_per_consumer = items / num_consumers;

            for _ in 0..num_consumers {
                let queue = &queue_arc;
                handles.push(s.spawn(move || {
                    let consumer = queue.consumer();
                    // Consume only the fair share of items per consumer
                    let mut count = 0;
                    while count < items_per_consumer {
                        if let Some((_, _)) = consumer.pop() {
                            count += 1;
                        }
                    }
                }));
            }

            // Wait for all threads to complete
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// Helper function to run the actual drain test with proper item distribution
    fn run_drain_test(items: usize, num_consumers: usize) {
        // Single producer fills queue (SPMC constraint)
        let mut queue = SPMCLockFreeQueue::<u64, 8192>::new(); // Reduced capacity
        for i in 0..items {
            let _ = queue.push(i as u64);
        }
        let queue_arc = Arc::new(queue);

        // Use std::thread::scope to avoid stack buildup from spawning threads
        std::thread::scope(|s| {
            let mut handles = vec![];
            // Calculate items per consumer to prevent infinite loops
            let items_per_consumer = items / num_consumers;

            for _ in 0..num_consumers {
                let queue = &queue_arc;
                handles.push(s.spawn(move || {
                    let consumer = queue.consumer();
                    // Consume only the fair share of items per consumer
                    let mut count = 0;
                    while count < items_per_consumer {
                        if consumer.pop().is_some() {
                            count += 1;
                        }
                    }
                }));
            }

            // Wait for all threads to complete
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    // ============ QUEUE COMPARISON ============

    #[test]
    fn bench_queue_comparison() {
        println!("\n=== QUEUE COMPARISON (10k sequential ops) ===");
        println!(
            "{:28} | {:>8} | {:>8} | {:>8} | {:>8} | {:>17}",
            "Benchmark", "Avg us", "p95 us", "Max us", "Min us", "Range ns"
        );
        println!("{}", "-".repeat(78));

        const OPS: usize = 10_000;

        benchmark_with_stats(&format!("spmc_{}k", OPS / 1000), RUNS / 2, || {
            let start = Instant::now();
            let mut queue = SPMCLockFreeQueue::<u64, 16384>::new();
            for i in 0..OPS {
                let _ = queue.push(i as u64);
            }
            let consumer = queue.consumer();
            while let Some((_, _)) = consumer.pop() {}
            start.elapsed().as_nanos()
        });
    }

    // ============ MEMORY FOOTPRINT ============

    #[test]
    fn bench_memory_footprint() {
        println!("\n=== MEMORY FOOTPRINT ===");

        use std::mem::size_of;

        let spmc_size = size_of::<SPMCLockFreeQueue<f64, 1024>>();

        println!("Struct overhead:");
        println!("  SPMC Queue<f64, 1024>:           {:>6} bytes", spmc_size);
        println!("");
        println!("Data buffer (per queue, pre-allocated):");
        println!("  1024 slots x 16 bytes = 16384 bytes = 16 KB");
    }
}

// Run with: cargo test --release --lib -- benchmarks -- --nocapture
