# AutoQueues Agent Instructions

## KISS PRINCIPLES (Keep It Simple, Stupid)

### Core KISS Philosophy

**Our KISS approach: Simple interfaces wrapping powerful, proven tools**

#### What KISS Means to Us

**✅ Clean Interfaces, Powerful Backends**
- Trait-based APIs that are simple to use
- Powerful libraries (QUIC, ZeroMQ, eval) doing the heavy lifting
- User sees simple interface, library handles complexity

**✅ Use Existing Tools, Don't Reinvent**
- Dependencies are leverage, not complexity
- Quinn, ZeroMQ, eval, tokio - proven libraries doing heavy lifting
- Your code stays minimal by leveraging existing solutions
- Don't build complex systems when good ones already exist

**✅ Comprehensive Validation as Good Engineering**
- "No validate should be fairly comprehensive" - catch issues early
- Validation prevents problems, doesn't add unnecessary complexity
- Better thorough validation than subtle runtime bugs
- Use existing validation libraries (eval) rather than building from scratch

**✅ Right Tool for the Right Job**
- Use the best available tool for each specific use case
- If ZeroMQ is better than QUIC, use ZeroMQ
- If QUIC is better than ZeroMQ, use QUIC
- Don't create artificial distinctions between "native" vs "external" communication
- Evaluate tools on their merits, not on preconceived use cases

**✅ Consistent Syntax and Conventions**
- Expression syntax matches topic syntax exactly
- `local.cpu_percent` in expressions → `local.cpu_percent` in topics
- No confusing conversions between different naming conventions
- Simple, predictable behavior throughout the system

**✅ Honest, Straightforward Communication**
- No inflated claims like "80% reduction"
- No emojis or marketing speak
- Clear, direct documentation
- Remove all BS statements

**✅ Consolidated, Focused Examples**
- One comprehensive example instead of scattered demos
- Show the system working as intended
- Reduce cognitive load for users

#### What KISS Does NOT Mean

❌ **NOT "few features"** - We can have powerful capabilities through simple interfaces
❌ **NOT "minimal validation"** - Comprehensive validation prevents issues
❌ **NOT "artificial tool distinctions"** - Don't create false categories for tools
❌ **NOT "no dependencies"** - Leverage existing proven libraries
❌ **NOT "simple systems"** - Simple to use systems with powerful capabilities
❌ **NOT "inconsistent syntax"** - Keep expression and topic syntax consistent

#### KISS Implementation Approaches

**We use several KISS-friendly approaches to achieve these principles:**

**1. Singleton Pattern for Runtime Simplicity**
```rust
// One implementation of each core component active at runtime
// Provides simple access and easy customization
impl NetworkManager {
    fn get() -> &'static NetworkManager {
        static MANAGER: OnceCell<NetworkManager> = OnceCell::new();
        MANAGER.get_or_init(|| NetworkManager::create())
    }
}
```

**2. Trait-Based Interfaces**
```rust
// Simple interface, powerful backend
pub trait Transport {
    fn connect(&self) -> Result<Connection, TransportError>;
    fn send(&self, data: &[u8]) -> Result<(), TransportError>;
}
```

**3. Leverage Existing Libraries**
```rust
// Use eval crate instead of building expression parser
use eval::Expr;
let expr = Expr::new(expression).eval()?;
```

**4. Consistent Expression-Topic Syntax**
```rust
// Expression: local.cpu_percent
// Topic: local.cpu_percent (same syntax!)
let expression = "(local.cpu_percent + local.memory_percent) / 2.0";
let topic = "local.cpu_percent"; // No conversion needed!
```

**5. Configuration-Driven Customization**
```rust
// Simple configuration enables "volia different impl of AutoQueues"
let config = AutoQueuesConfig {
    network: NetworkConfig {
        implementation: "zmq",  // or "quic" or custom
        // other settings...
    },
    // etc...
};

AutoQueues::initialize(config);
```

#### Easy Customization Pattern

**Users can easily extend the system through trait implementation:**

```rust
// 1. Implement the trait for custom functionality
impl Transport for MyCustomTransport {
    fn connect(&self) -> Result<Connection, TransportError> {
        // Custom connection logic
    }
    
    fn send(&self, data: &[u8]) -> Result<(), TransportError> {
        // Custom send logic  
    }
}

// 2. Add to configuration
let config = AutoQueuesConfig {
    network: NetworkConfig {
        implementation: "my_custom",
        custom_transport: Some(Box::new(MyCustomTransport)),
        // other settings...
    },
    // etc...
};

// 3. Initialize - Volia! Different implementation
AutoQueues::initialize(config);
```

#### KISS Assessment Criteria

When evaluating if something follows KISS:

1. **Interface Simplicity**: Is the API clean and easy to use?
2. **Tool Leverage**: Are we using existing proven libraries?
3. **Validation Adequacy**: Does validation catch real issues without over-engineering?
4. **Tool Selection**: Are we choosing the best tool for the job without artificial distinctions?
5. **Syntax Consistency**: Is the syntax consistent across expression and topics?
6. **Documentation Clarity**: Is communication honest and straightforward?
7. **Implementation Flexibility**: Can users easily customize components?

## INSTRUCTIONS

- Keep this document updated with any changes to code structure, style guidelines, or architecture.
- Never use EMOJIS
- Always adhere to the KISS principle (Keep It Simple, Stupid)
- Always write clear, concise comments in code
- Always write clear commit messages describing changes
- always follow Rust best practices and idioms
- always keep it simple and efficient
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
- Error types: `QueueError`, specific variants for different failures

### Expression Engine

#### Consistent Variable Syntax

```rust
// Expression syntax MATCHES topic syntax - no conversion needed!
// Valid local variable references
"local.cpu_percent"           // → topic "local.cpu_percent"
"local.memory_usage"          // → topic "local.memory_usage"
"local.drive_capacity"        // → topic "local.drive_capacity"

// Valid math expressions
"(local.cpu_percent + local.memory_percent) / 2.0"
"local.drive_percent.sqrt() * 10.0"
"local.memory_percent.pow(1.5) / 10.0"
"local.cpu_percent.abs().max(local.memory_percent)"
```

#### Mathematical Functions Supported

- Basic arithmetic: `+ - * /`
- Power functions: `.pow(exponent)`
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

### KISS-Friendly Architecture

**Strategic Decision**: Simple interfaces wrapping powerful, proven tools

- **Trait-Based Design**: Clean interfaces for easy implementation
- **Proven Libraries**: QUIC, ZeroMQ, eval, tokio for heavy lifting
- **Configuration-Driven**: Easy customization through simple config
- **Runtime Simplicity**: Singleton pattern for predictable behavior
- **Consistent Syntax**: Expression and topic syntax match exactly

#### Core Components

**Network Layer:**
- QUIC or ZeroMQ (whichever proves better for the use case)
- Custom implementations via trait
- Configuration-driven choice
- No artificial "native" vs "external" distinctions

**Queue System:**
- Generic queue implementation supporting any data type
- Expression evaluation integration
- Pub/sub messaging coordination
- Simple, focused API

**Expression Engine:**
- Mathematical expression evaluation using eval library
- Variable substitution using consistent syntax
- Comprehensive validation for reliability
- Custom function support

**System Metrics:**
- Real-time CPU, memory, disk collection via sysinfo
- Health score calculations
- Cross-platform support
- Custom metric providers

#### Architecture Pattern

```rust
// KISS architecture - simple interfaces, powerful backends
pub trait Transport {
    fn connect(&self) -> Result<Connection, TransportError>;
    fn send(&self, data: &[u8]) -> Result<(), TransportError>;
    fn receive(&self) -> Result<Vec<u8>, TransportError>;
}

pub struct NetworkManager {
    transport: Box<dyn Transport>,
}

impl NetworkManager {
    // Simple interface wrapping powerful transport
    pub fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        let transport = match config.implementation.as_str() {
            "quic" => Box::new(QuicTransport::new(config)) as Box<dyn Transport>,
            "zmq" => Box::new(ZmqTransport::new(config)) as Box<dyn Transport>,
            "custom" => config.custom_transport.unwrap(),
        };
        
        Ok(NetworkManager { transport })
    }
    
    // Clean API - user doesn't need to know transport details
    pub async fn send(&self, data: &[u8]) -> Result<(), NetworkError> {
        self.transport.send(data)
    }
}
```

#### Configuration System

```rust
// Simple configuration enables powerful customization
pub struct AutoQueuesConfig {
    pub network: NetworkConfig,
    pub queue: QueueConfig,
    pub expression: ExpressionConfig,
    pub metrics: MetricsConfig,
    pub pubsub: PubSubConfig,
}

pub struct NetworkConfig {
    pub implementation: String,  // "quic", "zmq", or custom
    pub custom_transport: Option<Box<dyn Transport>>,
    pub timeout: Duration,
    pub max_connections: usize,
}

// Easy initialization
let config = AutoQueuesConfig::from_file("autoqueues.toml")?;
AutoQueues::initialize(config)?;
```

## Expression Engine Architecture

### Local-Only Scope

Queues can ONLY evaluate `local.*` expressions. Global variables will be handled by Queue Manager.

### Expression Processing Flow

1. Queue receives expression: `"(local.cpu_percent + local.memory_percent) / 2"`
2. System extracts `local.*` variables automatically
3. Maps variables to topics using SAME syntax: `local.cpu_percent` → `local.cpu_percent`
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

- `local.cpu_percent` - Local system CPU percentage
- `metrics.component.hostname` - Component metrics by host
- `global.aggregate` - Global aggregated metrics (Queue Manager)
- `alerts.severity` - System alerts by severity

### Wildcard Patterns

- `local.*` - All local metrics
- `metrics.*.system_a` - All component metrics for system_a
- `alerts.high` - Only high-severity alerts

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

### Singleton Access Pattern

```rust
// Clean, simple access to core components
let network = NetworkManager::get();
let queue = QueueManager::get();
let expression = ExpressionEngine::get();
let metrics = MetricsCollector::get();
let pubsub = PubSubBroker::get();
let config = ConfigManager::get();
```

## Integration Patterns

### Multi-Queue Aggregation

```rust
// CPU metrics queue (publishes to topic with consistent syntax)
let cpu_queue = QueueBuilder::new()
    .expression("local.cpu_percent")
    .publish_to("local.cpu_percent")
    .build();

// Health aggregation queue (subscribes to multiple topics)
let health_queue = QueueBuilder::new()
    .expression("(local.cpu_percent + local.memory_percent) / 2")
    .subscribe_to("local.*")
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

### Network Configuration

```rust
// KISS network configuration - use best tool for the job
let config = NetworkConfig {
    implementation: "zmq",  // or "quic" - choose the best tool
    timeout: Duration::from_secs(30),
    max_connections: 1000,
};

let manager = NetworkManager::with_config(config);
```

### Complete System Initialization

```rust
// Single point of initialization for entire system
let config = AutoQueuesConfig::from_file("autoqueues.toml")?;
AutoQueues::initialize(config)?;

// All components are now initialized and ready
let network = NetworkManager::get();
let queue = QueueManager::get();
// etc...
```

## Performance Considerations

### Memory Efficiency

- Zero-copy where possible in pub/sub
- Single instance of each component (no duplication)
- Shared Arc<RwLock<>> for concurrent access
- Efficient topic routing with HashMap

### Latency Targets

- Local pub/sub: < 1ms within machine
- Expression evaluation: < 1ms for math operations
- Metric collection: Real-time via sysinfo
- Network transport: < 5ms (varies by implementation)
- Singleton access: < 0.1ms for component retrieval

### Network Performance

- **QUIC**: High-performance networking when appropriate
- **ZeroMQ**: Broad compatibility and reliability when appropriate
- **Single Stack**: No overhead from unused network protocols
- **Configuration**: Easy to switch network types for different environments

### Singleton Performance Benefits

- **Thread-safe initialization**: OnceCell ensures single initialization
- **Static lifetime**: Singletons live for program duration
- **Lock-free access**: Most singleton operations are lock-free
- **Minimal overhead**: Singleton pattern adds negligible performance cost

## Testing Strategy

### Expression Testing

```rust
#[test]
fn test_local_variable_parsing() {
    let var = LocalVariable::new("local.cpu_percent");
    assert_eq!(var.topic, "local.cpu_percent"); // Consistent syntax!
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
    let receiver = broker.subscribe_exact("local.cpu_percent".to_string()).await?;
    broker.publish("local.cpu_percent".to_string(), &data).await?;
    // Verify message received...
}
```

### Network Testing

```rust
#[test]
fn test_network_singleton() {
    // Ensure only one network type is active
    let manager1 = NetworkManager::get();
    let manager2 = NetworkManager::get();
    assert_eq!(manager1 as *const _, manager2 as *const _);
}
```

### Complete System Testing

```rust
#[test]
fn test_singleton_architecture() {
    // Test that all singletons are properly initialized
    let network = NetworkManager::get();
    let queue = QueueManager::get();
    let expression = ExpressionEngine::get();
    let metrics = MetricsCollector::get();
    let pubsub = PubSubBroker::get();
    let config = ConfigManager::get();
    
    // Verify all singletons are accessible and non-null
    assert!(!network.is_null());
    assert!(!queue.is_null());
    assert!(!expression.is_null());
    assert!(!metrics.is_null());
    assert!(!pubsub.is_null());
    assert!(!config.is_null());
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

### Network Evolution

- Easy to add new network protocols as they emerge
- Maintain KISS principle: simple interface, powerful backend
- Configuration-driven network selection
- Plugin architecture for network transports

### Plugin System

- Dynamic loading of custom implementations
- Plugin registry for third-party extensions
- Configuration-based plugin activation
- Version compatibility checking

### Advanced Configuration

- Environment-based configuration overrides
- Hot-reloading of configuration changes
- Configuration validation and schema evolution
- Multi-profile configuration management

This architecture provides a solid foundation for distributed queue management while delivering sophisticated local processing capabilities through simple, clean interfaces and proven tooling.

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