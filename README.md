# AutoQueues Documentation

AutoQueues is a high-performance distributed queue system designed for HPC environments.

## Programmatic Mode (Library)

```rust
use autoqueues::config::ConfigGenerator;
use autoqueues::AutoQueues;

let config = ConfigGenerator::local_test(1, 6967);
let queues = AutoQueues::new(config);

queues.add_queue_fn::<f64, _>("cpu", || 42.0)?;
queues.start();

let value = queues.try_pop::<f64>("cpu")?;
```

## Performance

| Operation | Latency |
|-----------|----------|
| SPMCLockFreeQueue push | ~0.05μs |
| SPMCLockFreeQueue pop | ~0.04μs |
| ShardedRingBuffer push/pop | ~0.5μs |

## Installation

```toml
[dependencies]
autoqueues = "0.1"
```

## Core Concepts

### Queues

AutoQueues provides several queue implementations:

- **ShardedRingBuffer**: Lock-free queue using atomic operations with pre-allocated ring buffer
- **SPMCLockFreeQueue**: Single-Producer, Multiple-Consumer lock-free queue

### Queue Operations

```rust
// Async operations (default)
queues.push("data", 42.5).await?;
let value = queues.pop("data").await?;

// Sync operations (faster)
queues.try_push("data", 42.5)?;
let value = queues.try_pop("data")?;
```

### Expressions

Expressions support mathematical operations on metrics:

```rust
// Queue with expression filter
let queues = AutoQueues::new(config)
    .add_queue_expr(
        "health_score", 
        "(cpu + memory) / 2.0",
        "system_metrics",
        true,  // trigger_on_push
        Some(1000)  // evaluate every 1000ms
    )?;

// Expression functions
// - Arithmetic: +, -, *, /, ^ (power)
// - Comparisons: >, >=, <, <=, ==, !=
// - Logic: && (and), || (or), ! (not)
// - Math functions: sqrt(x), abs(x), pow(x, n), round(x), floor(x), ceil(x)
// - Trig functions: sin(x), cos(x), tan(x)
// - Aggregations: max(a, b, c...), min(a, b, c...)
// - Time windows: avg(cpu, 5000), max(cpu, 60000)
```

### Pub/Sub

```rust
// Subscribe to topics
let sub = queues.subscribe("alerts.*").await?;

// Publish to topics
queues.publish_to_topic("alerts.cpu", &85.5).await?;

// Receive messages
while let Some(msg) = queues.receive(sub).await {
    println!("Received: {:?}", msg);
}
```

## Architecture

### Project Structure

```
src/
├── autoqueues.rs          # Main programmatic API
├── config.rs              # Configuration handling
├── expression/mod.rs      # Expression engine (single file)
├── network/
│   └── pubsub/
│       └── zmq.rs         # ZMQ-based pub/sub
├── queue/
│   ├── lockfree.rs            # ShardedRingBuffer
│   ├── spmc_lockfree_queue.rs # SPMCLockFreeQueue (true lock-free)
│   └── registry.rs            # Queue registry
├── node.rs                # Node management
└── types.rs               # Core types
```

### Design Principles

1. **KISS**: Simple interfaces, powerful backends
2. **Trait-based**: Clean abstractions with multiple implementations
3. **Performance first**: Sub-microsecond operations where possible
4. **Consistent syntax**: Expression variables match topic syntax

### Performance Optimizations

- Integer-based queue identification (faster than string keys)
- Pre-allocated circular buffers (no heap allocation)
- Sharded registry (16x reduced lock contention)
- Atomic operations for simple counters
- Sync methods for hot paths

## Configuration

```toml
[queues]
base = ["cpu_usage", "memory_usage"]

[queues.derived.health_score]
formula = "(cpu_usage + memory_usage) / 2.0"
```

## Examples

See the `examples/` directory:

- `01_basic.rs` - Basic API usage
- `01_basic_usage.rs` - Simple function-based queues
- `local_node.rs` - Running a distributed node
- `config_test.rs` - Configuration testing

## Testing

```bash
# Run all tests
cargo test --lib

# Run specific tests
cargo test --lib autoqueues
cargo test --lib queue
```

## Dependencies

See `Cargo.toml` for full dependency list.

## License

MIT OR Apache-2.0