# AutoQueues Documentation

AutoQueues is a high-performance distributed queue system designed for HPC environments.

## Two APIs

### Server Mode (Redis-like)

```rust
use autoqueues::AutoQueuesServer;

// From configuration file
let server = AutoQueuesServer::from_file("config.toml").await?;

// Or with minimal configuration
let server = AutoQueuesServer::minimal()
    .port(6969)
    .push_topic("metrics.cpu")
    .subscribe_topic("alerts.*")
    .build()?;

server.run().await;
```

### Programmatic Mode (Library)

```rust
let queues: AutoQueues<f64> = AutoQueues::new()
    .queue("metrics", Some("atomic.cpu_percent > 80"))?
    .build()
    .await?;

queues.push("metrics", 85.5).await?;
let value = queues.pop("metrics").await?;
```

## Performance

| Operation | Latency |
|-----------|----------|
| LockFreeQueue push | ~0.05μs |
| LockFreeQueue pop | ~0.04μs |
| AutoQueues try_push | ~0.5μs |
| AutoQueues try_pop | ~0.5μs |
| Async push/pop | ~0.8μs |

## Installation

```toml
[dependencies]
autoqueues = "0.1"
```

## Core Concepts

### Queues

AutoQueues provides several queue implementations:

- **SimpleQueue**: Basic queue with circular buffer
- **LockFreeQueue**: Lock-free queue using atomic operations

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
let queues = AutoQueues::new()
    .add_queue_expr(
        "health_score", 
        "(local.cpu + local.memory) / 2.0",
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
├── autoqueues.rs      # Main programmatic API
├── server.rs          # Server mode implementation
├── config.rs          # Configuration handling
├── expression/        # Expression engine
│   ├── types.rs       # AST node definitions
│   ├── parser.rs      # Expression parser
│   └── evaluator.rs   # Expression evaluator
├── pubsub.rs         # Publish/subscribe system
├── queue/
│   ├── implementation.rs  # SimpleQueue
│   ├── lockfree.rs       # LockFreeQueue
│   └── registry.rs       # Sharded registry
├── queue_manager.rs  # Queue management
├── traits/           # Trait definitions
└── types.rs          # Core types
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
formula = "(atomic.cpu_usage + atomic.memory_usage) / 2.0"
```

## Examples

See the `examples/` directory:

- `programmatic_mode.rs` - Main API usage
- `server_mode.rs` - Server mode example
- `lockfree_queue_example.rs` - Lock-free queue demo
- `performance_benchmark.rs` - Performance benchmarks

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
