# AutoQueues Agent Instructions

## Build & Test Commands

### Basic Operations
```bash
# Build library
cargo build --lib

# Run specific test
cargo test test_name --lib

# Check compilation without running
cargo check --lib

# Build all examples
cargo build --examples

# Run specific example
cargo run --example example_name
```

### Advanced Examples
```bash
# Run real system monitoring with actual metrics
cargo run --example real_system_monitoring

# Test mathematical expression engine
cargo run --example expression_engine_demo

# Test publisher-subscriber system
cargo run --example pubsub_demo

# Test health score calculations
cargo run --example health_score_demo

# Test new Quinn-based distributed networking
cargo run --example quinn_networking_demo
```

### Debug & Development
```bash
# Check specific example only
cargo check --example expression_engine_demo

# Run tests with output  
cargo test --verbose

# Profile build times
cargo build --timings

# Test distributed quorum functionality
cargo test test_quorum_consensus --lib

# Benchmark Quinn transport performance
cargo bench --bench transport_benchmarks
```

## Code Style Guidelines

### Imports
- Use absolute imports: `use crate::module::Type`
- Group imports: std, external, internal
define types, then public functions
- Function naming: `snake_case` with action verb
- Error types: `QueueError`, specific variants for different failures

### Expression Engine
#### Local Variable Syntax
```rust
// Valid local variable references
"local.cpu_percent"           // → topic "local/cpu-percent"
"local.memory_usage"          // → topic "local/memory-usage" 
"local.drive_capacity"        // → topic "local/drive-capacity"

// Valid math expressions
"(local.cpu_percent + local.memory_percent) / 2.0"
"local.drive_percent.sqrt() * 10.0"
"local.memory_percent.powf(1.5) / 10.0"
"local.cpu_percent.abs().max(local.memory_percent)"
```

#### Mathematical Functions Supported
- Basic arithmetic: `+ - * /`
- Power functions: `.powf(exponent)`
- Roots: `.sqrt()`
- Absolute value: `.abs()`
- Maximum/Minimum: `.max(other)`, `.min(other)`
- Rounding: `.round()`, `.ceil()`, `.floor()`

### Error Handling
- Use `Result<T, QueueError>` for fallible operations
- Provide specific error messages: `QueueError::Other("Context: actual issue")`
- Validate inputs early, return errors with context
- Division by zero → default to 0 in expressions

### Async/await
- All queue operations are async
- Use `tokio::spawn` for background tasks
- Handle concurrent access with Arc<Mutex<T>> appropriately

### Testing
- Unit tests in module: `#[cfg(test)] mod tests { }`
- Test names: `test_function_name_scenario`
- Test empty, single, multiple data scenarios

## Architecture Overview

### Stratified Networking (New QUIC-Only Foundation)

**Strategic Decision**: Quinn-based QUIC transport for **both** control and data planes

- **Control Plane**: Reliable coordination (Raft, global variables, health monitoring)
- **Data Plane**: High-throughput queue streaming (continuous data flow)
- **Single Foundation**: Both planes share Quinn connection, different optimization strategies

```rust
// Stratified transport architecture
pub trait Transport { /* shared connection management */ }
pub trait ControlPlaneTransport: Transport { /* coordination patterns */ }
pub trait DataPlaneTransport: Transport { /* streaming patterns */ }

// Single factory creates both plane types from same connection
impl TransportFactory for QuinnTransportFactory {
    fn create_control_transport(&self, conn: Connection) -> QuinnControlTransport;
    fn create_data_transport(&self, conn: Connection) -> QuinnDataTransport;
}
```

#### Plane-Specific Optimizations
- **Control Plane**: Optimized for low-latency coordination (< 5ms)
- **Data Plane**: Optimized for high-throughput streaming (95-98% of raw TCP)
- **Shared Connection**: Resource efficiency via stream multiplexing
- **Buffer Management**: Plane-specific buffer strategies optimize memory usage

## Expression Engine Architecture

### Local-Only Scope
Queues can ONLY evaluate `local.*` expressions. Global variables will be handled by Queue Manager.

### Expression Processing Flow
1. Queue receives expression: `"(local.cpu + local.memory) / 2"`
2. System extracts `local.*` variables automatically
3. Maps variables to topics: `local.cpu_percent` → `local/cpu-percent`
4. Queue subscribes to all required topics
5. When any topic updates, recalculates expression at queue interval
6. Returns computed result with division-by-zero protection

### Expression Configuration
```rust
let queue = QueueBuilder::new()
    .expression("(local.drive_percent + local.cpu_percent) / 2.0")
    .interval(1000) // Evaluate every 1000ms
    .build();
```

## Pub/Sub System

### Topic Hierarchy
- `local/{metric-type}` - Local system metrics
- `metrics/{component}/{hostname}` - Component metrics by host
- `global/{aggregate}` - Global aggregated metrics (Queue Manager)
- `alerts/{severity}` - System alerts by severity

### Wildcard Patterns
- `local/*` - All local metrics
- `metrics/*/system_a` - All component metrics for system_a
- `alerts/high` - Only high-severity alerts

### Subscription Modes
- **Exact**: Subscribe to specific topic
- **Pattern**: Subscribe using wildcards
- **Filtered**: Subscribe with conditions

## API Design Principles

### Local-First Architecture
- All processing starts local
- Distributed coordination handled by separate Queue Manager
- Global variables (global.*) resolved by Queue Manager
- Local variables (local.*) resolved by local queues

### Helper Methods Priority
Always prefer the new helper methods over raw access:
```rust
// ✅ Preferred - Clean and safe
let latest_value = queue.get_latest_value().await;
let recent_values = queue.get_latest_n_values(5).await;

// ❌ Old way - Verbose and error-prone
let latest = queue.get_latest().await.map(|(_, data)| data);
```

### Error Resilience
- Expression errors don't crash queues
- Division by zero returns 0, not panic
- Missing data uses safe defaults
- Failed subscriptions handled gracefully

### Stratified Networking Implementation
- Control plane errors trigger retry logic and coordination recovery
- Data plane errors trigger backpressure and flow control adjustments
- Transport errors propagate differently per plane (control=retry, data=backpressure)
- Connection failures clean up both planes atomically

## Integration Patterns

### Multi-Queue Aggregation
```rust
// CPU metrics queue (publishes to topic)
let cpu_queue = QueueBuilder::new()
    .expression("local.cpu_usage")
    .publish_to("local/cpu_usage")
    .build();

// Health aggregation queue (subscribes to multiple topics)
let health_queue = QueueBuilder::new()
    .expression("(local.cpu_usage + local.memory_usage) / 2")
    .subscribe_to("local/*")
    .build();
```

### Real System Integration
```rust
use autoqueues::metrics::{MetricsCollector, SystemMetrics};

let mut collector = MetricsCollector::new();
let cpu_usage = collector.get_cpu_usage();
let memory = collector.get_memory_usage(); 
let health_score = collector.get_health_score();
```

### Distributed Networking Integration (New)
```rust
// Quinn-based distributed coordination
let cluster = QueueManager::new()
    .with_transport_factory(QuinnTransportFactory::new(quinn_config))
    .join_cluster("cluster-alpha".to_string())
    .await?;

// Control plane: Raft coordination for global variables
let global_cpu = cluster.create_global_metric(
    "cluster_avg_cpu",
    local_cpu_queue,
    AggregationType::Average
).await?;

// Data plane: High-throughput queue streaming
let data_stream = cluster.create_inter_node_stream(
    target_node,
    StreamConfig::high_throughput()
).await?;
```

## Performance Considerations

### Memory Efficiency
- Zero-copy where possible in pub/sub
- Shared Arc<RwLock<>> for concurrent access
- Efficient topic routing with HashMap

### Latency Targets
- Local pub/sub: < 1ms within machine
- Expression evaluation: < 1ms for math operations
- Metric collection: Real-time via sysinfo
- Quinn transport streams: < 5ms intra-cluster

### Stratified Networking Performance
- **Control Plane**: Optimized for low-latency coordination (< 5ms)
- **Data Plane**: Optimized for high-throughput streaming (95-98% of raw TCP)
- **Shared Connection**: Resource efficiency via stream multiplexing
- **Buffer Management**: Plane-specific buffer strategies optimize memory usage

## Testing Strategy

### Expression Testing
```rust
#[test]
fn test_local_variable_parsing() {
    let var = LocalVariable::new("local.cpu_percent");
    assert_eq!(var.topic, "local/cpu-percent");
}

#[test] 
fn test_expression_evaluation() {
    let expr = LocalExpression::new("local.a + local.b").unwrap();
    // Setup context variables...
    let result = expr.evaluate().unwrap();
    assert!(result > 0.0);
}
```

### Pub/Sub Testing
```rust
#[test]
fn test_topic_subscription() {
    let broker = PubSubBroker::new(100);
    let receiver = broker.subscribe_exact("local/test".to_string()).await?;
    broker.publish("local/test".to_string(), &data).await?;
    // Verify message received...
}
```

### Stratified Networking Testing (New)
```rust
#[test]
fn test_control_plane_reliability() {
    // Test Raft message delivery guarantees
    // Test timeout and retry behavior
    // Test coordination under network stress
}

#[test]
fn test_data_plane_throughput() {
    // Test high-throughput queue streaming
    // Test backpressure mechanisms
    // Test buffer management under load
}
```

## Future Extensions

### Queue Manager Integration
- Handle global.* variable resolution across network
- Coordinate cross-host topic subscriptions
- Aggregate global metrics across clusters
- Support user expressions involving both local and global variables

### Advanced Math Functions
- Statistical functions (mean, median, std_dev)
- Time-series analysis (trend, seasonality)
- Machine learning integration
- Custom user-defined functions

### Stratified Networking Evolution
- QUIC protocol optimizations for queue-specific patterns
- Adaptive buffer management based on queue characteristics
- Dynamic stream prioritization based on queue importance
- Advanced flow control for heterogenous network conditions

This architecture provides a solid foundation for distributed queue management while delivering sophisticated local processing capabilities immediately.

## Expression Engine Architecture

### Local-Only Scope
Queues can ONLY evaluate `local.*` expressions. Global variables will be handled by Queue Manager.

### Expression Processing Flow
1. Queue receives expression: `"(local.cpu + local.memory) / 2"`
2. System extracts `local.*` variables automatically
3. Maps variables to topics: `local.cpu_percent` → `local/cpu-percent`
4. Queue subscribes to all required topics
5. When any topic updates, recalculates expression at queue interval
6. Returns computed result with division-by-zero protection

### Expression Configuration
```rust
let queue = QueueBuilder::new()
    .expression("(local.drive_percent + local.cpu_percent) / 2.0")
    .interval(1000) // Evaluate every 1000ms
    .build();
```

## Pub/Sub System

### Topic Hierarchy
- `local/{metric-type}` - Local system metrics
- `metrics/{component}/{hostname}` - Component metrics by host
- `global/{aggregate}` - Global aggregated metrics (Queue Manager)
- `alerts/{severity}` - System alerts by severity

### Wildcard Patterns
- `local/*` - All local metrics
- `metrics/*/system_a` - All component metrics for system_a
- `alerts/high` - Only high-severity alerts

### Subscription Modes
- **Exact**: Subscribe to specific topic
- **Pattern**: Subscribe using wildcards
- **Filtered**: Subscribe with conditions

## API Design Principles

### Local-First Architecture
- All processing starts local
- Distributed coordination handled by separate Queue Manager
- Global variables (global.*) resolved by Queue Manager
- Local variables (local.*) resolved by local queues

### Helper Methods Priority
Always prefer the new helper methods over raw access:
```rust
// ✅ Preferred - Clean and safe
let latest_value = queue.get_latest_value().await;
let recent_values = queue.get_latest_n_values(5).await;

// ❌ Old way - Verbose and error-prone
let latest = queue.get_latest().await.map(|(_, data)| data);
```

### Error Resilience
- Expression errors don't crash queues
- Division by zero returns 0, not panic
- Missing data uses safe defaults
- Failed subscriptions handled gracefully

## Integration Patterns

### Multi-Queue Aggregation
```rust
// CPU metrics queue (publishes to topic)
let cpu_queue = QueueBuilder::new()
    .expression("local.cpu_usage")
    .publish_to("local/cpu_usage")
    .build();

// Health aggregation queue (subscribes to multiple topics)
let health_queue = QueueBuilder::new()
    .expression("(local.cpu_usage + local.memory_usage) / 2")
    .subscribe_to("local/*")
    .build();
```

### Real System Integration
```rust
use autoqueues::metrics::{MetricsCollector, SystemMetrics};

let mut collector = MetricsCollector::new();
let cpu_usage = collector.get_cpu_usage();
let memory = collector.get_memory_usage(); 
let health_score = collector.get_health_score();
```

## Performance Considerations

### Memory Efficiency
- Zero-copy where possible in pub/sub
- Shared Arc<RwLock<>> for concurrent access
- Efficient topic routing with HashMap

### Latency Targets
- Local pub/sub: < 1ms within machine
- Expression evaluation: < 1ms for math operations
- Metric collection: Real-time via sysinfo

## Testing Strategy

### Expression Testing
```rust
#[test]
fn test_local_variable_parsing() {
    let var = LocalVariable::new("local.cpu_percent");
    assert_eq!(var.topic, "local/cpu-percent");
}

#[test] 
fn test_expression_evaluation() {
    let expr = LocalExpression::new("local.a + local.b").unwrap();
    // Setup context variables...
    let result = expr.evaluate().unwrap();
    assert!(result > 0.0);
}
```

### Pub/Sub Testing
```rust
#[test]
fn test_topic_subscription() {
    let broker = PubSubBroker::new(100);
    let receiver = broker.subscribe_exact("local/test".to_string()).await?;
    broker.publish("local/test".to_string(), &data).await?;
    // Verify message received...
}
```

## Future Extensions

### Queue Manager Integration
- Handle global.* variable resolution across network
- Coordinate cross-host topic subscriptions
- Aggregate global metrics across clusters
- Support user expressions involving both local and global variables

### Advanced Math Functions
- Statistical functions (mean, median, std_dev)
- Time-series analysis (trend, seasonality)
- Machine learning integration
- Custom user-defined functions

This architecture provides a solid foundation for distributed queue management while delivering sophisticated local processing capabilities immediately.