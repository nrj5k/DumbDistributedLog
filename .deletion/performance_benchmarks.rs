use autoqueues::*;
use std::sync::Arc;
use criterion::{criterion_group, criterion_main, Criterion};

// Simple throughput benchmark
fn throughput_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    
    group.bench_function("publish_100_items", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let queue_type = QueueType::new(
                    QueueValue::Sim,
                    10,
                    2,
                    "benchmark.log".to_string(),
                    "benchmark_var".to_string(),
                );

                let config = QueueConfig::new(
                    Mode::Sensor,
                    Arc::new(|_| Ok(42)),
                    Model::Linear,
                    queue_type,
                );

                let mut queue = Queue::new(config);
                for i in 0..100 {
                    queue.publish(i).await.unwrap();
                }
            });
        });
    });
    
    group.finish();
}

criterion_group!(benches, throughput_benchmark);
criterion_main!(benches);