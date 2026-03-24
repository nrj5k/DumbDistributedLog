//! DDL Performance Benchmarks
//!
//! Measures throughput, latency, and scalability.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ddl::{Ddl, DdlConfig, DDLTrait, DdlWithWal};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark single-threaded push throughput
fn bench_push_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("push_throughput");
    group.throughput(Throughput::Elements(1000));
    group.measurement_time(Duration::from_secs(10));
    
    group.bench_function("in_memory_1000_entries", |b| {
        b.to_async(&rt).iter(|| async {
            let ddl = Ddl::default();
            
            for i in 0..1000 {
                let data = format!("entry_{}", i);
                let _ = black_box(ddl.push("test_topic", data.into_bytes()).await.unwrap());
            }
        });
    });
    
    group.finish();
}

/// Benchmark push latency (single entry)
fn bench_push_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("push_latency", |b| {
        b.to_async(&rt).iter(|| async {
            let ddl = Ddl::default();
            let data = vec![0u8; 100]; // 100 bytes
            let _ = black_box(ddl.push("test_topic", data.clone()).await);
        });
    });
}

/// Benchmark subscribe throughput
fn bench_subscribe_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("subscribe_throughput");
    group.throughput(Throughput::Elements(1000));
    
    group.bench_function("consume_1000_entries", |b| {
        b.to_async(&rt).iter(|| async {
            let ddl = Ddl::default();
            
            // Pre-populate
            for i in 0..1000 {
                let _ = ddl.push("test_topic", format!("entry_{}", i).into_bytes()).await.unwrap();
            }
            
            // Subscribe and consume
            let mut stream = ddl.subscribe("test_topic").await.unwrap();
            let mut count = 0;
            while let Some(entry) = stream.next() {
                black_box(entry);
                count += 1;
                if count >= 1000 {
                    break;
                }
            }
        });
    });
    
    group.finish();
}

/// Benchmark different payload sizes
fn bench_payload_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("payload_sizes");
    
    for size in [100, 1024, 10240, 102400].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_function(format!("{}_bytes", size), |b| {
            let ddl = Ddl::default();
            let data = vec![0u8; *size];
            
            b.to_async(&rt).iter(|| async {
                let _ = black_box(ddl.push("test_topic", data.clone()).await.unwrap());
            });
        });
    }
    
    group.finish();
}

/// Benchmark ring buffer wraparound (edge case)
fn bench_ring_buffer_wraparound(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("ring_buffer_wraparound", |b| {
        b.to_async(&rt).iter(|| async {
            // Small buffer to force wraparound
            let config = DdlConfig {
                buffer_size: 100,
                ..Default::default()
            };
            let ddl = Ddl::new(config);
            
            // Write more than buffer size
            for i in 0..200 {
                let _ = black_box(ddl.push("test_topic", vec![i as u8]).await.unwrap());
            }
        });
    });
}

/// Benchmark concurrent producers
fn bench_concurrent_producers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_producers");
    group.throughput(Throughput::Elements(1000));
    
    for num_producers in [2, 4, 8, 16].iter() {
        group.bench_function(format!("{}_producers", num_producers), |b| {
            b.to_async(&rt).iter(|| async {
                let ddl = Ddl::default();
                let ddl = std::sync::Arc::new(ddl);
                
                let mut handles = vec![];
                
                for p in 0..*num_producers {
                    let ddl_clone = ddl.clone();
                    let handle = tokio::spawn(async move {
                        for i in 0..(1000 / num_producers) {
                            let data = format!("producer_{}_entry_{}", p, i);
                            let _ = ddl_clone.push("test_topic", data.into_bytes()).await.unwrap();
                        }
                    });
                    handles.push(handle);
                }
                
                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    }
    
    group.finish();
}

/// Benchmark with WAL enabled
fn bench_with_wal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("wal_comparison");
    group.throughput(Throughput::Elements(100));
    
    // Without WAL
    group.bench_function("in_memory_only", |b| {
        b.to_async(&rt).iter(|| async {
            let ddl = Ddl::default();
            for i in 0..100 {
                let _ = black_box(ddl.push("test_topic", vec![i as u8]).await.unwrap());
            }
        });
    });
    
    // With WAL (slower but durable)
    group.bench_function("with_wal", |b| {
        b.to_async(&rt).iter(|| async {
            use tempfile::TempDir;
            
            let temp_dir = TempDir::new().unwrap();
            let config = DdlConfig {
                data_dir: temp_dir.path().to_path_buf(),
                wal_enabled: true,
                ..Default::default()
            };
            let ddl = DdlWithWal::new(config, temp_dir.path()).unwrap();
            
            for i in 0..100 {
                let _ = black_box(ddl.push("test_topic", vec![i as u8]).await.unwrap());
            }
        });
    });
    
    group.finish();
}

/// Benchmark TCP vs ZMQ transport (simulated)
fn bench_transport_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("transport_overhead");
    
    // Local (no network)
    group.bench_function("local_only", |b| {
        b.to_async(&rt).iter(|| async {
            let ddl = Ddl::default();
            let _ = black_box(ddl.push("test_topic", vec![1, 2, 3]).await.unwrap());
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_push_throughput,
    bench_push_latency,
    bench_subscribe_throughput,
    bench_payload_sizes,
    bench_ring_buffer_wraparound,
    bench_concurrent_producers,
    bench_with_wal,
    bench_transport_overhead
);

criterion_main!(benches);