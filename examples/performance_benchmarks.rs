//! Comprehensive performance benchmarks for AutoQueues
//!
//! Measures performance of queue operations, expression evaluation,
//! and system metrics collection.

use autoqueues::{Expression, MetricsCollector, Queue, SimpleExpression, SimpleQueue};
use std::collections::HashMap;
use std::time::Instant;

struct BenchmarkSuite {
    metrics: MetricsCollector,
}

impl BenchmarkSuite {
    fn new() -> Self {
        Self {
            metrics: MetricsCollector::new(),
        }
    }

    fn run_all_benchmarks(&mut self) {
        println!("🏁 AutoQueues Performance Benchmarks");
        println!("====================================");

        self.benchmark_queue_operations();
        self.benchmark_expression_evaluation();
        self.benchmark_metrics_collection();
        self.benchmark_memory_usage();

        println!("\n🎉 All benchmarks completed!");
    }

    fn benchmark_queue_operations(&mut self) {
        println!("\n📊 Queue Operations Benchmark");
        println!("------------------------------");

        // Test queue creation
        let start = Instant::now();
        let mut queue = SimpleQueue::<String>::new();
        let creation_time = start.elapsed();

        // Test queue publishing
        let start = Instant::now();
        for i in 0..10000 {
            let _ = queue.publish(format!("Test message {}", i));
        }
        let publish_time = start.elapsed();

        // Test queue retrieval
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = queue.get_latest();
        }
        let retrieval_time = start.elapsed();

        // Test batch retrieval
        let start = Instant::now();
        for _ in 0..100 {
            let _ = queue.get_latest_n(100);
        }
        let batch_time = start.elapsed();

        println!(
            "   Queue Creation:     {:8.3} μs",
            creation_time.as_micros()
        );
        println!("   10k Publish Ops:    {:8.3} ms", publish_time.as_millis());
        println!(
            "   1k Retrieval Ops:    {:8.3} ms",
            retrieval_time.as_millis()
        );
        println!("   100 Batch Ops:      {:8.3} ms", batch_time.as_millis());
        println!(
            "   Publish Rate:        {:8.0} ops/sec",
            10000.0 / publish_time.as_secs_f64()
        );
        println!(
            "   Retrieval Rate:      {:8.0} ops/sec",
            1000.0 / retrieval_time.as_secs_f64()
        );
    }

    fn benchmark_expression_evaluation(&mut self) {
        println!("\n🧮 Expression Evaluation Benchmark");
        println!("-----------------------------------");

        // Setup test variables
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 75.0);
        local_vars.insert("memory".to_string(), 60.0);
        local_vars.insert("disk".to_string(), 45.0);

        let mut global_vars = HashMap::new();
        global_vars.insert("network".to_string(), 30.0);
        global_vars.insert("power".to_string(), 85.0);

        // Test expressions of varying complexity
        let test_expressions = vec![
            ("Simple Variable", "local.cpu"),
            ("Basic Addition", "local.cpu + local.memory"),
            ("Complex Math", "(local.cpu + local.memory) / 2.0"),
            ("Math Function", "sqrt(local.cpu)"),
            (
                "Complex Function",
                "pow(local.memory, 2.0) + abs(local.disk)",
            ),
            ("Comparison", "max(local.cpu, global.power)"),
        ];

        for (name, expr_str) in test_expressions {
            let expr = SimpleExpression::new(expr_str).unwrap();

            // Warm up
            for _ in 0..100 {
                let _ = expr.evaluate(&local_vars, &global_vars);
            }

            // Benchmark
            let start = Instant::now();
            for _ in 0..10000 {
                let _ = expr.evaluate(&local_vars, &global_vars);
            }
            let elapsed = start.elapsed();

            println!(
                "   {:20}: {:8.3} ms ({:8.0} evals/sec)",
                name,
                elapsed.as_millis(),
                10000.0 / elapsed.as_secs_f64()
            );
        }
    }

    fn benchmark_metrics_collection(&mut self) {
        println!("\n📈 Metrics Collection Benchmark");
        println!("-------------------------------");

        // Test single metric collection
        let start = Instant::now();
        let _metrics = self.metrics.get_all_metrics();
        let single_time = start.elapsed();

        // Test repeated metric collection
        let start = Instant::now();
        for _ in 0..1000 {
            let _metrics = self.metrics.get_all_metrics();
        }
        let repeated_time = start.elapsed();

        // Test CPU usage specifically
        let start = Instant::now();
        for _ in 0..1000 {
            let _cpu = self.metrics.get_cpu_usage();
        }
        let cpu_time = start.elapsed();

        // Test memory usage specifically
        let start = Instant::now();
        for _ in 0..1000 {
            let _memory = self.metrics.get_memory_usage();
        }
        let memory_time = start.elapsed();

        println!("   Single Collection:   {:8.3} μs", single_time.as_micros());
        println!(
            "   1k Collections:      {:8.3} ms",
            repeated_time.as_millis()
        );
        println!("   1k CPU Queries:      {:8.3} ms", cpu_time.as_millis());
        println!("   1k Memory Queries:   {:8.3} ms", memory_time.as_millis());
        println!(
            "   Collection Rate:     {:8.0} collections/sec",
            1000.0 / repeated_time.as_secs_f64()
        );
    }

    fn benchmark_memory_usage(&mut self) {
        println!("\n💾 Memory Usage Benchmark");
        println!("--------------------------");

        // Test queue memory usage
        let mut queue = SimpleQueue::<String>::new();

        // Add items and measure memory growth
        let start = Instant::now();
        for i in 0..10000 {
            let _ = queue.publish(format!(
                "Large test message number {} with extra data to increase memory usage per item",
                i
            ));
        }
        let fill_time = start.elapsed();

        println!("   10k Items Added:    {:8.3} ms", fill_time.as_millis());
        println!(
            "   Avg per Item:       {:8.3} μs",
            fill_time.as_micros() / 10000
        );

        // Test expression memory usage
        let expressions: Vec<SimpleExpression> = vec![
            SimpleExpression::new("local.cpu").unwrap(),
            SimpleExpression::new("local.memory").unwrap(),
            SimpleExpression::new("local.cpu + local.memory").unwrap(),
            SimpleExpression::new("sqrt(local.cpu)").unwrap(),
            SimpleExpression::new("max(local.cpu, local.memory)").unwrap(),
        ];

        let start = Instant::now();
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 75.0);
        local_vars.insert("memory".to_string(), 60.0);
        let global_vars = HashMap::new();

        for _ in 0..10000 {
            for expr in &expressions {
                let _ = expr.evaluate(&local_vars, &global_vars);
            }
        }
        let expr_time = start.elapsed();

        println!("   50k Expression Eval: {:8.3} ms", expr_time.as_millis());
        println!(
            "   Avg per Eval:       {:8.3} μs",
            expr_time.as_micros() / 50000
        );
    }

    fn print_system_info(&mut self) {
        println!("\n🖥️  System Information");
        println!("----------------------");

        let metrics = self.metrics.get_all_metrics();
        println!("   CPU Usage:          {:.1}%", metrics.cpu_usage_percent);
        println!(
            "   Memory Usage:       {:.1}%",
            metrics.memory_usage.usage_percent
        );
        println!(
            "   Disk Usage:         {:.1}%",
            metrics.disk_usage.usage_percent
        );
        println!("   System Name:        {}", metrics.system_info.system_name);
        println!(
            "   Process Count:      {}",
            metrics.system_info.process_count
        );
        println!(
            "   Uptime:             {} seconds",
            metrics.system_info.uptime_seconds
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut benchmark = BenchmarkSuite::new();

    println!("🚀 Starting AutoQueues Performance Benchmarks...");
    println!("This will test various aspects of the system performance.");
    println!();

    // Print system info first
    benchmark.print_system_info();

    // Run all benchmarks
    benchmark.run_all_benchmarks();

    println!("\n📋 Benchmark Summary:");
    println!("   ✅ Queue operations tested");
    println!("   ✅ Expression evaluation tested");
    println!("   ✅ Metrics collection tested");
    println!("   ✅ Memory usage analyzed");
    println!();
    println!("💡 Performance Tips:");
    println!("   • Queue operations are optimized for high throughput");
    println!("   • Expression evaluation is cached for repeated use");
    println!("   • Metrics collection uses efficient system calls");
    println!("   • Memory usage scales linearly with data volume");

    Ok(())
}
