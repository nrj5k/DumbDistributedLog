# DDL Agent Instructions

## DDL API: Trait-Based Design

DDL provides a clean, simple trait-based API for distributed log operations.

### DDL Trait (Core Interface)

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlTcp::new(config));

// Push data to a topic
ddl.push("metrics.cpu", &data).await?;

// Subscribe to topics matching a pattern
let stream = ddl.subscribe("metrics.*").await?;

// Acknowledge processing for at-least-once delivery
ddl.ack("metrics.cpu", entry_id).await?;
```

Features:
- Simple, focused trait-based API
- Multiple implementations (TCP, in-memory, mock)
- Push/subscribe/ack operations
- Shard assignment via Raft
- Clear error types

## DDL Trait Definition

```rust
/// Core trait for DDL implementations.
#[async_trait]
pub trait DDL {
    /// Push data to a topic
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError>;

    /// Subscribe to topics matching a pattern
    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError>;

    /// Acknowledge an entry (for at-least-once delivery)
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;

    /// Create a new topic with specified capacity
    async fn create_topic(&self, topic: &str, capacity: usize) -> Result<(), DdlError>;
}

/// Trait for subscription streams
#[async_trait]
pub trait SubscriptionStream {
    /// Receive next entry
    async fn next(&mut self) -> Result<Option<Entry>, DdlError>;

    /// Acknowledge processing
    async fn ack(&mut self, entry_id: u64) -> Result<(), DdlError>;

    /// Subscribe to additional patterns
    async fn add_pattern(&mut self, pattern: &str) -> Result<(), DdlError>;
}
```

## Implementations

### DdlTcp
- TCP transport for distributed deployment
- Full cluster support with Raft shard assignment
- Production-ready

### DdlInMemory
- In-memory implementation for testing
- No network overhead
- Single-node only

### MockDDL
- For unit testing
- Programmable expectations
- No real I/O

## KISS PRINCIPLES FOR DDL API

The API follows KISS principles:
- Simple interfaces wrapping powerful backends
- Consistent naming (topics, entries)
- Minimal boilerplate
- Good defaults with override capability
- Clear documentation

### Core KISS Philosophy

**Our KISS approach: Simple interfaces wrapping powerful, proven tools**

#### What KISS Means to Us

**Clean Interfaces, Powerful Backends**
- Trait-based APIs that are simple to use
- Powerful libraries (Raft, TCP) doing the heavy lifting
- User sees simple interface, library handles complexity

**Use Existing Tools, Don't Reinvent**
- Dependencies are leverage, not complexity
- Raft, TCP, tokio - proven libraries doing heavy lifting
- Your code stays minimal by leveraging existing solutions
- Don't build complex systems when good ones already exist

**Comprehensive Validation as Good Engineering**
- "No validate should be fairly comprehensive" - catch issues early
- Validation prevents problems, doesn't add unnecessary complexity
- Better thorough validation than subtle runtime bugs
- Use existing validation libraries rather than building from scratch

**Right Tool for the Right Job**
- Use the best available tool for each specific use case
- If TCP is better than QUIC, use TCP
- If QUIC is better than TCP, use QUIC
- Don't create artificial distinctions between "native" vs "external" communication
- Evaluate tools on their merits, not on preconceived use cases

**Honest, Straightforward Communication**
- No inflated claims like "80% reduction"
- No emojis or marketing speak
- Clear, direct documentation
- Remove all BS statements

**Consolidated, Focused Examples**
- One comprehensive example instead of scattered demos
- Show the system working as intended
- Reduce cognitive load for users

#### What KISS Does NOT Mean

NOT "few features" - We can have powerful capabilities through simple interfaces
NOT "minimal validation" - Comprehensive validation prevents issues
NOT "artificial tool distinctions" - Don't create false categories for tools
NOT "no dependencies" - Leverage existing proven libraries
NOT "simple systems" - Simple to use systems with powerful capabilities
NOT "inconsistent syntax" - Keep topic syntax consistent

## KISS Implementation Approaches

**We use several KISS-friendly approaches to achieve these principles:**

### 1. Trait-Based Interfaces
```rust
// Simple interface, powerful backend
#[async_trait]
pub trait DDL {
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError>;
    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError>;
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;
}
```

### 2. Multiple Implementations
```rust
// Different backends, same trait
pub struct DdlTcp { ... }     // Distributed deployment
pub struct DdlInMemory { ... } // Testing/single-node
#[cfg(test)]
pub struct MockDDL { ... }    // Unit testing
```

### 3. Leverage Existing Libraries
```rust
// Use Raft for shard assignment
use openraft::Raft;

// Use tokio for async
use tokio::spawn;
```

### 4. Configuration-Driven Customization
```rust
// Simple configuration enables different implementations
let config = DdlConfig {
    transport: TransportConfig::Tcp {
        host: "0.0.0.0",
        port: 6969,
    },
    cluster: ClusterConfig {
        peers: vec!["node1:6969", "node2:6969"],
    },
};
```

## Easy Customization Pattern

**Users can easily extend the system through trait implementation:**

```rust
// 1. Implement the trait for custom functionality
#[async_trait]
impl DDL for MyCustomDDL {
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError> {
        // Custom push logic
    }

    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError> {
        // Custom subscribe logic
    }

    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError> {
        // Custom ack logic
    }
}

// 2. Use the custom implementation
let ddl: Arc<dyn DDL> = Arc::new(MyCustomDDL::new());
```

## KISS Assessment Criteria

When evaluating if something follows KISS:

1. **Interface Simplicity**: Is the API clean and easy to use?
2. **Tool Leverage**: Are we using existing proven libraries?
3. **Validation Adequacy**: Does validation catch real issues without over-engineering?
4. **Tool Selection**: Are we choosing the best tool for the job without artificial distinctions?
5. **Syntax Consistency**: Is the syntax consistent across topics and operations?
6. **Documentation Clarity**: Is communication honest and straightforward?
7. **Implementation Flexibility**: Can users easily customize components?

## INSTRUCTIONS

- Keep this document updated with any changes to code structure, style guidelines, or architecture.
- Never use EMOJIS
- Always adhere to the KISS principle (Keep It Simple, Stupid)
- Always write clear, concise comments in code
- Always write clear commit messages describing changes
- Always follow Rust best practices and idioms
- Always keep it simple and efficient
- Never make MOCKS for production code
- Write thorough tests for all new features and bug fixes
- Never prematurely optimize, profile first
- Never celebrate until tests pass and code is clean

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

### Main Example

```bash
# Run the comprehensive demonstration
cargo run --example comprehensive_demo
```

### Debug & Development

```bash
# Check specific example only
cargo check --example comprehensive_demo

# Run tests with output
cargo test --verbose

# Profile build times
cargo build --timings

# Benchmark performance
cargo bench --lib
```

## Code Style Guidelines

### Imports

- Use absolute imports: `use crate::module::Type`
- Group imports: std, external, internal
- Define types, then public functions
- Function naming: `snake_case` with action verb
- Error types: `DdlError`, specific variants for different failures

### Error Handling

- Use `Result<T, DdlError>` for fallible operations
- Provide specific error messages: `DdlError::ClusterError("Context: actual issue")`
- Validate inputs early, return errors with context
- Clear error variants for different failure modes

### Async/await

- All DDL operations are async
- Use `tokio::spawn` for background tasks
- Handle concurrent access with Arc<Mutex<T>> appropriately

### Testing

- Unit tests in module: `#[cfg(test)] mod tests { }`
- Test names: `test_function_name_scenario`
- Test empty, single, multiple data scenarios
- Use MockDDL for controlled testing

## Architecture Overview

### KISS-Friendly Architecture

**Strategic Decision**: Simple interfaces wrapping powerful, proven tools

- **Trait-Based Design**: Clean interfaces for easy implementation
- **Proven Libraries**: Raft, TCP, tokio for heavy lifting
- **Configuration-Driven**: Easy customization through simple config
- **Consistent Syntax**: Topic names consistent across operations

#### Core Components

**Network Layer:**
- TCP transport for reliable messaging
- Custom implementations via trait
- Simple, focused protocol

**Topic System:**
- Lock-free SPMC ring buffers
- Pre-allocated circular buffers
- Monotonic entry IDs
- At-least-once acknowledgment

**Cluster Coordination:**
- Raft for shard assignment only
- Shard map management
- Node membership tracking

#### Architecture Pattern

```rust
// KISS architecture - simple interfaces, powerful backends
#[async_trait]
pub trait DDL {
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError>;
    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError>;
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;
}

pub struct DdlTcp {
    config: DdlConfig,
    shard_map: Arc<RwLock<ShardMap>>,
    topics: DashMap<String, Topic>,
}

#[async_trait]
impl DDL for DdlTcp {
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError> {
        // Simple implementation wrapping powerful primitives
        let entry = Entry::new(topic, payload);
        self.topics.get(topic).push(entry)
    }

    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError> {
        // Pattern matching, stream creation
        let stream = SubscriptionStreamImpl::new(pattern);
        Ok(Box::new(stream))
    }

    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError> {
        // Acknowledgment processing
        self.topics.get(topic).ack(entry_id)
    }
}
```

#### Configuration System

```rust
// Simple configuration enables powerful customization
pub struct DdlConfig {
    pub transport: TransportConfig,
    pub cluster: ClusterConfig,
    pub topic_defaults: TopicConfig,
}

pub struct TransportConfig {
    pub implementation: String,  // "tcp", "in-memory", or custom
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

// Easy initialization
let config = DdlConfig::from_file("ddl.toml")?;
let ddl: Arc<dyn DDL> = Arc::new(DdlTcp::new(config));
```

## Performance Considerations

### Memory Efficiency

- Zero-copy where possible in topic operations
- Pre-allocated ring buffers
- Single instance of each component (no duplication)
- Shared Arc<RwLock<>> for concurrent access
- Efficient topic routing with HashMap

### Latency Targets

- Local push: < 10μs (lock-free)
- Local subscribe receive: < 5μs
- Network push: < 100μs
- Raft commit (shard map): < 10ms
- Shard migration: < 50ms

### Network Performance

- **TCP**: Reliable messaging for distributed deployment
- **In-memory**: Zero overhead for testing
- **Single Stack**: No overhead from unused network protocols
- **Configuration**: Easy to switch implementations for different environments

## Testing Strategy

### Unit Testing

```rust
#[test]
fn test_topic_push_pop_roundtrip() {
    let topic = Topic::new();
    let data = b"test_payload";
    let entry_id = topic.push(data).unwrap();
    let entry = topic.pop().unwrap();
    assert_eq!(entry.payload, data);
}

#[tokio::test]
async fn test_ddl_trait_push() {
    let mock_ddl = MockDDL::new();
    mock_ddl.expect_push().returning(|_, _| Ok(1));
    let result = mock_ddl.push("test", b"data").await;
    assert!(result.is_ok());
}
```

### Integration Testing

```rust
#[tokio::test]
async fn test_ddl_integration() {
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

    // Create topic
    ddl.create_topic("test.topic", 1000).await.unwrap();

    // Subscribe
    let mut stream = ddl.subscribe("test.*").await.unwrap();

    // Push data
    let entry_id = ddl.push("test.topic", b"hello").await.unwrap();

    // Receive
    let entry = stream.next().await.unwrap();
    assert_eq!(entry.topic, "test.topic");
    assert_eq!(entry.payload, b"hello");

    // Acknowledge
    stream.ack(entry.id).await.unwrap();
}
```

### Mock Testing

```rust
use mockall::predicate::*;
use ddl::DDL;

#[tokio::test]
async fn test_with_mock_ddl() {
    let mut mock = MockDDL::new();
    mock.expect_push()
        .with(eq("metrics.cpu"), always())
        .returning(|_, _| Ok(1));
    mock.expect_subscribe()
        .with(eq("metrics.*"))
        .returning(|_| Ok(Box::new(MockStream::new())));

    // Test using mock
    let result = mock.push("metrics.cpu", b"data").await;
    assert!(result.is_ok());
}
```

## Honest Assessment

DDL provides:

- **Yes**: Simple distributed log with push/subscribe/ack
- **Yes**: Trait-based API for flexibility
- **Yes**: Raft shard assignment
- **Yes**: Multiple implementations

DDL does NOT provide:

- **No**: Expression evaluation (build separately if needed)
- **No**: Metrics collection (use system tools)
- **No**: Automatic failover (design for it separately)
- **No**: Consumer groups (build on DDL trait)
- **No**: Message replay (build on DDL trait)

## Future Extensions

Extensions can build on DDL trait:

1. **Expression Engine**
   - Using evalexpr or fasteval
   - Optional dependency
   - Separate crate

2. **Consumer Groups**
   - Shared offsets
   - Message replay
   - Built on DDL trait

3. **Schema Registry**
   - Message validation
   - Schema evolution
   - Optional feature

4. **Persistent Storage**
   - Write-ahead logging
   - Topic compaction
   - Recovery support

This architecture provides a solid foundation for distributed logging while enabling extensions through the clean DDL trait interface.

## HPC & ML/AI Systems Best Practices

### Performance-Critical Rules

**Memory Management**
- Zero-copy architectures whenever possible
- Prefer stack allocation over heap
- Use arena allocators for frequent allocations
- Minimize pointer indirection

**Cache Efficiency**
- Structure data for cache-friendly access patterns
- Use contiguous memory layouts
- Align data structures to cache line boundaries
- Consider prefetching for predictable access patterns

**Parallel Processing**
- Design for data parallelism first
- Use thread pools instead of spawning threads
- Minimize lock contention with lock-free structures
- Consider NUMA awareness for multi-socket systems

### Rust-Specific HPC Patterns

**Unsafe Judiciously**
- Use unsafe only for proven performance bottlenecks
- Document safety invariants clearly
- Prefer `unsafe` blocks over entire functions
- Benchmark safe alternatives first

**Type System for Performance**
- Use `#[repr(C)]` for predictable memory layout
- Leverage `const` generics for compile-time optimization
- Use `PhantomData` for zero-cost abstractions
- Prefer monomorphization over dynamic dispatch

**Memory Layout**
- `Vec<T>` for contiguous storage
- `SmallVec` for stack allocation in common cases
- `Cow<str>` for borrow-or-clone patterns
- `Box<[T]>` for fixed-size heap arrays

### ML/AI Systems Rules

**Data Pipeline Efficiency**
- Stream processing when possible
- Minimize data movement between CPU/GPU
- Use pinned memory for GPU transfers
- Batch operations for better utilization

**Numerical Stability**
- Use appropriate precision (f32 vs f64)
- Implement numerically stable algorithms
- Handle edge cases (NaN, Inf) explicitly
- Validate mathematical operations

### Code Quality

**Benchmark Everything**
- Profile before optimizing
- Use criterion for microbenchmarks
- Test with realistic data sizes
- Monitor memory usage patterns

**Error Handling**
- Use `Result<T>` for recoverable errors
- `panic` only for unrecoverable conditions
- Avoid exception handling in hot paths
- Consider error codes for performance-critical code

The key is: **measure first, optimize second**. HPC rewards empirical optimization over theoretical improvements.