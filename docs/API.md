# DDL API Reference

**Complete API reference for the Dumb Distributed Log system.**

## Table of Contents

1. [Core Traits](#core-traits)
2. [DDL Trait Methods](#ddl-trait-methods)
3. [EntryStream Methods](#entrystream-methods)
4. [Configuration](#configuration)
5. [Error Types](#error-types)
6. [Transport Configuration](#transport-configuration)
7. [Usage Examples](#usage-examples)

---

## Core Traits

### DDL Trait

The `DDL` trait is the primary interface for all distributed log operations. It provides push, subscribe, and acknowledgment functionality.

```rust
use async_trait::async_trait;

#[async_trait]
pub trait DDL: Send + Sync {
    /// Push data to a topic
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError>;

    /// Subscribe to topics matching a pattern
    async fn subscribe(&self, pattern: &str) -> Result<EntryStream, DdlError>;

    /// Acknowledge an entry has been processed
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;

    /// Get current position for a topic
    async fn position(&self, topic: &str) -> Result<u64, DdlError>;

    /// Check if this node owns a topic
    fn owns_topic(&self, topic: &str) -> bool;
}
```

**Thread Safety:**

- The trait is `Send + Sync` for safe concurrent access
- Implementations must be thread-safe
- All methods are async and can be called from any task

### Entry Stream Trait

For more advanced subscription scenarios, the `EntryStream` trait provides additional control:

```rust
#[async_trait]
pub trait EntryStream {
    /// Receive the next entry (blocking)
    async fn next(&mut self) -> Result<Option<Entry>, DdlError>;

    /// Try to receive the next entry (non-blocking)
    fn try_next(&self) -> Result<Option<Entry>, DdlError>;

    /// Acknowledge processing of an entry
    async fn ack(&mut self, entry_id: u64) -> Result<(), DdlError>;

    /// Subscribe to additional patterns
    async fn add_pattern(&mut self, pattern: &str) -> Result<(), DdlError>;
}
```

---

## DDL Trait Methods

### push

Push data to a topic and receive a unique entry ID.

```rust
async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError>
```

**Parameters:**

- `topic`: The topic name to push to (e.g., `"metrics.cpu"`)
- `payload`: The data to store (any `Vec<u8>`)

**Returns:**

- `Ok(u64)`: The unique entry ID for this push
- `Err(DdlError::NotOwner)`: If this node doesn't own the topic
- `Err(DdlError::BufferFull)`: If the topic buffer is full
- `Err(DdlError::Network)`: If there's a network error

**Example:**

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

// Push simple data
let entry_id = ddl.push("metrics.cpu", vec![1, 2, 3, 4]).await?;
println!("Pushed entry with ID: {}", entry_id);

// Push JSON data
let data = serde_json::to_vec(&serde_json::json!({
    "cpu": 45.2,
    "memory": 8192
}))?;
let entry_id = ddl.push("metrics.system", data).await?;
```

**Behavior:**

- The entry is appended to the topic's ring buffer
- A monotonically increasing ID is assigned
- If WAL is enabled, the entry is written to disk before returning
- The call blocks until the entry is durably stored (if WAL enabled)

---

### subscribe

Subscribe to topics matching a pattern and receive an entry stream.

```rust
async fn subscribe(&self, pattern: &str) -> Result<EntryStream, DdlError>
```

**Parameters:**

- `pattern`: Topic pattern with glob-style wildcards (e.g., `"metrics.*"`)

**Returns:**

- `Ok(EntryStream)`: A stream for receiving matching entries
- `Err(DdlError::TopicNotFound)`: If no topics match the pattern

**Pattern Syntax:**

- `*` matches any sequence of characters (e.g., `"metrics.*"` matches `"metrics.cpu"`, `"metrics.memory"`)
- `?` matches any single character (e.g., `"metrics.?"` matches `"metrics.c"` but not `"metrics.cpu"`)
- Patterns are case-sensitive

**Example:**

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

// Create the topic first (required for new topics)
ddl.create_topic("metrics.cpu", 1000).await?;
ddl.create_topic("metrics.memory", 1000).await?;

// Subscribe to all metrics
let mut stream = ddl.subscribe("metrics.*").await?;

// Receive entries
while let Some(entry) = stream.next().await {
    println!("Topic: {}, ID: {}, Payload: {:?}",
             entry.topic, entry.id, entry.payload);

    // Acknowledge processing
    stream.ack(entry.id).await?;
}
```

**Behavior:**

- Creates connections to all nodes owning matching topics
- Streams entries from all matching topics in parallel
- Entries are interleaved as they arrive
- The stream remains open until explicitly dropped

---

### ack

Acknowledge processing of an entry, enabling buffer space reclamation.

```rust
async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>
```

**Parameters:**

- `topic`: The topic containing the entry
- `entry_id`: The ID of the entry to acknowledge

**Returns:**

- `Ok(())`: Acknowledgment successful
- `Err(DdlError::NotOwner)`: If this node doesn't own the topic
- `Err(DdlError::EntryNotFound)`: If the entry ID is not found

**Example:**

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

let mut stream = ddl.subscribe("metrics.*").await?;

while let Some(entry) = stream.next().await {
    // Process the entry
    process_entry(&entry);

    // Acknowledge when done
    ddl.ack(&entry.topic, entry.id).await?;
}
```

**Behavior:**

- Marks the entry as processed
- Advances the acknowledgment cursor
- Allows buffer space to be reclaimed
- Does not delete the entry immediately (lazy cleanup)

---

### position

Get the current position (last entry ID) for a topic.

```rust
async fn position(&self, topic: &str) -> Result<u64, DdlError>
```

**Parameters:**

- `topic`: The topic to query

**Returns:**

- `Ok(u64)`: The current position (last entry ID)
- `Err(DdlError::TopicNotFound)`: If the topic doesn't exist

**Example:**

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

// Create a topic
ddl.create_topic("metrics.cpu", 1000).await?;

// Get current position
let pos = ddl.position("metrics.cpu").await?;
println!("Current position: {}", pos); // Outputs: Current position: 0

// Push some data
ddl.push("metrics.cpu", vec![1, 2, 3]).await?;
ddl.push("metrics.cpu", vec![4, 5, 6]).await?;

// Get updated position
let pos = ddl.position("metrics.cpu").await?;
println!("Current position: {}", pos); // Outputs: Current position: 2
```

---

### owns_topic

Check if this node owns a specific topic.

`fn owns_topic(&self, topic: &str) -> bool`

**Parameters:**

- `topic`: The topic to check

**Returns:**

- `bool`: `true` if this node owns the topic, `false` otherwise

**Example:**

```rust
use ddl::DDL;

let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

ddl.create_topic("metrics.cpu", 1000).await?;

if ddl.owns_topic("metrics.cpu") {
    println!("This node owns metrics.cpu");
} else {
    println!("This node does NOT own metrics.cpu");
}
```

---

## EntryStream Methods

### next

Receive the next entry from the stream (blocking).

```rust
async fn next(&mut self) -> Result<Option<Entry>, DdlError>
```

**Returns:**

- `Ok(Some(Entry))`: The next entry
- `Ok(None)`: The stream has been closed
- `Err(DdlError::Network)`: If there's an error receiving

**Example:**

```rust
let mut stream = ddl.subscribe("metrics.*").await?;

while let Some(entry) = stream.next().await {
    println!("Received: {:?}", entry);
}
```

---

### try_next

Try to receive the next entry (non-blocking).

```rust
fn try_next(&self) -> Result<Option<Entry>, DdlError>
```

**Returns:**

- `Ok(Some(Entry))`: An entry is available
- `Ok(None)`: No entry available currently
- `Err(DdlError::Network)`: If there's an error

**Example:**

```rust
loop {
    if let Some(entry) = stream.try_next()? {
        println!("Received: {:?}", entry);
    } else {
        // No entry available, do other work
        tokio::task::yield_now().await;
    }
}
```

---

### ack

Acknowledge processing of an entry (via stream).

```rust
async fn ack(&mut self, entry_id: u64) -> Result<(), DdlError>
```

**Parameters:**

- `entry_id`: The ID of the entry to acknowledge

**Example:**

```rust
while let Some(entry) = stream.next().await {
    process_entry(&entry);
    stream.ack(entry.id).await?;
}
```

---

## Configuration

### DdlConfig

The main configuration structure for DDL instances:

```rust
#[derive(Debug, Clone)]
pub struct DdlConfig {
    /// Unique ID for this node
    pub node_id: u64,
    /// List of peer nodes (for gossip)
    pub peers: Vec<String>, // "host:port" format
    /// Topics this node owns (static assignment for now)
    pub owned_topics: Vec<String>,
    /// Ring buffer size per topic
    pub buffer_size: usize,
    /// Whether to enable gossip discovery
    pub gossip_enabled: bool,
    /// Address to bind gossip to
    pub gossip_bind_addr: String,
    /// Bootstrap peers for gossip
    pub gossip_bootstrap: Vec<String>,
    /// Data directory for WAL and persistence
    pub data_dir: std::path::PathBuf,
    /// Whether to enable WAL durability
    pub wal_enabled: bool,
}
```

**Default Values:**

```rust
impl Default for DdlConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            peers: vec![],
            owned_topics: vec![],
            buffer_size: 1024 * 1024, // 1MB default
            gossip_enabled: false,
            gossip_bind_addr: "0.0.0.0:0".to_string(),
            gossip_bootstrap: vec![],
            data_dir: std::path::PathBuf::from("/tmp/ddl"),
            wal_enabled: true,
        }
    }
}
```

**Example:**

```rust
use ddl::DdlConfig;

let config = DdlConfig {
    node_id: 1,
    peers: vec!["node2:6969".to_string(), "node3:6969".to_string()],
    owned_topics: vec!["metrics.cpu".to_string(), "metrics.memory".to_string()],
    buffer_size: 1024 * 1024,
    gossip_enabled: true,
    gossip_bind_addr: "0.0.0.0:7001".to_string(),
    gossip_bootstrap: vec!["node1:7001".to_string()],
    data_dir: std::path::PathBuf::from("/data/ddl"),
    wal_enabled: true,
};
```

---

### Transport Configuration

#### TcpConfig

Configuration for TCP transport:

```rust
pub struct TcpConfig {
    /// Listen address
    pub bind_addr: SocketAddr,
    /// Maximum connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keepalive_interval: Duration,
}
```

#### ZmqConfig

Configuration for ZMQ transport:

```rust
pub struct ZmqConfig {
    /// ZMQ context
    pub context: zmq::Context,
    /// Publish endpoint
    pub publish_endpoint: String,
    /// Subscribe endpoint
    pub subscribe_endpoint: String,
    /// Message batch size
    pub batch_size: usize,
    /// Flush interval
    pub flush_interval: Duration,
}
```

#### HybridConfig

Configuration for hybrid transport:

```rust
pub struct HybridConfig {
    /// TCP configuration for control plane
    pub tcp: TcpConfig,
    /// ZMQ configuration for data plane
    pub zmq: ZmqConfig,
}
```

---

### Persistence Configuration

#### PersistenceConfig

Configuration for disk persistence:

```rust
pub struct PersistenceConfig {
    /// Data directory
    pub data_dir: PathBuf,
    /// Flush interval
    pub flush_interval: Duration,
    /// Maximum buffer size before forced flush
    pub max_buffer_size: usize,
    /// Enable compression
    pub compression: bool,
    /// Recovery on startup
    pub recover_on_startup: bool,
}
```

---

## Error Types

### DdlError

All errors returned by DDL operations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DdlError {
    #[error("Topic {0} not owned by this node")]
    NotOwner(String),

    #[error("Topic {0} not found")]
    TopicNotFound(String),

    #[error("Entry {0} not found in topic {1}")]
    EntryNotFound(u64, String),

    #[error("Buffer full for topic {0}")]
    BufferFull(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}
```

**Error Handling Examples:**

```rust
use ddl::{DDL, DdlError};

match ddl.push("metrics.cpu", data).await {
    Ok(entry_id) => println!("Pushed entry: {}", entry_id),
    Err(DdlError::NotOwner(topic)) => {
        // Redirect to correct node
        redirect_to_owner(topic);
    }
    Err(DdlError::BufferFull(topic)) => {
        // Wait and retry or increase buffer
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(DdlError::Network(e)) => {
        // Handle network issues
        reconnect();
    }
    Err(e) => {
        // Other errors
        panic!("Unexpected error: {}", e);
    }
}
```

---

## Transport Configuration

### Selecting Transport Type

DDL supports multiple transport implementations. Choose based on your requirements:

**TCP Transport** - Default, production-ready:

```rust
use ddl::transport::TcpTransport;

let transport = TcpTransport::new("0.0.0.0:6969").await?;
```

**ZMQ Transport** - High performance:

```rust
use ddl::transport::ZmqTransport;

let transport = ZmqTransport::new("tcp://*:6969")?;
```

**Hybrid Transport** - Best of both:

```rust
use ddl::transport::HybridTransport;

let transport = HybridTransport::new(
    TcpConfig::new("0.0.0.0:6969"),
    ZmqConfig::new("tcp://*:6968"),
)?;
```

---

## Usage Examples

### Complete Minimal Example

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create DDL instance
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

    // Create a topic
    ddl.create_topic("metrics.cpu", 10000).await?;

    // Push some data
    let entry_id = ddl.push("metrics.cpu", vec![1, 2, 3, 4]).await?;
    println!("Pushed entry: {}", entry_id);

    // Subscribe and receive
    let mut stream = ddl.subscribe("metrics.cpu").await?;
    while let Some(entry) = stream.next().await {
        println!("Received: {:?}", entry);
        stream.ack(entry.id).await?;
    }

    Ok(())
}
```

### Complete Cluster Example

```rust
use ddl::{DDL, DdlDistributed, DdlConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DdlConfig {
        node_id: 1,
        peers: vec!["node2:6969".to_string(), "node3:6969".to_string()],
        owned_topics: vec!["metrics.cpu".to_string()],
        buffer_size: 1024 * 1024,
        gossip_enabled: true,
        gossip_bind_addr: "0.0.0.0:7001".to_string(),
        gossip_bootstrap: vec!["node1:7001".to_string()],
        data_dir: std::path::PathBuf::from("/data/ddl"),
        wal_enabled: true,
    };

    let ddl: Arc<dyn DDL> = Arc::new(DdlDistributed::new(config).await?);

    // Start listening
    ddl.listen("0.0.0.0:6969").await?;

    // Run forever
    std::future::pending::<()>().await;

    Ok(())
}
```

### Using with Different Implementations

```rust
use ddl::{DDL, DdlInMemory, DdlWal, DdlDistributed};
use std::sync::Arc;

// In-memory (fast, no persistence)
let in_memory: Arc<dyn DDL> = Arc::new(DdlInMemory::new());

// WAL-backed (durable)
let wal: Arc<dyn DDL> = Arc::new(DdlWal::new("/data/ddl").await?);

// Distributed (cluster-aware)
let distributed: Arc<dyn DDL> = Arc::new(DdlDistributed::new(config).await?);
```

### Error Handling Pattern

```rust
use ddl::{DDL, DdlError};
use std::sync::Arc;

async fn safe_push(
    ddl: &Arc<dyn DDL>,
    topic: &str,
    payload: Vec<u8>,
    max_retries: u32,
) -> Result<u64, DdlError> {
    let mut retries = 0;

    loop {
        match ddl.push(topic, payload.clone()).await {
            Ok(entry_id) => return Ok(entry_id),
            Err(DdlError::BufferFull(_)) if retries < max_retries => {
                // Backoff and retry
                tokio::time::sleep(Duration::from_millis(100 << retries)).await;
                retries += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

---

## Summary

The DDL API provides:

- **Simple three-operation interface**: push, subscribe, ack
- **Pattern-based subscriptions**: match topics with wildcards
- **Multiple implementations**: in-memory, WAL, distributed
- **Comprehensive error types**: clear error messages with context
- **Flexible configuration**: customize transport, storage, and behavior

All operations are async and thread-safe, suitable for high-concurrency environments.

