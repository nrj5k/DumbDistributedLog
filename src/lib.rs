//! DDL - Dumb Distributed Log
//!
//! A minimal distributed append-only log for HPC clusters with ultra-fast queue operations.
//!
//! # Overview
//!
//! DDL provides a simple, trait-based API for distributed log operations:
//!
//! - **Push**: Append data to a topic
//! - **Subscribe**: Receive entries from topics matching a pattern
//! - **Ack**: Acknowledge processing for at-least-once delivery
//!
//! # Architecture
//!
//! The library is organized into several modules with clean separation of concerns:
//!
//! - **Core** ([`ddl`]): In-memory DDL implementation with lock-free SPMC queues
//! - **Distributed** ([`ddl_distributed`]): Network-aware DDL with TCP transport
//! - **WAL** ([`wal`], [`ddl_wal`]): Write-ahead logging for persistence
//! - **Network** ([`network`]): Transport abstractions (TCP, ZMQ, Hybrid)
//! - **Cluster** ([`cluster`]): Raft-based coordination for shard assignment
//! - **Queue** ([`queue`]): Lock-free SPMC ring buffers with backpressure
//! - **Gossip** ([`gossip`]): P2P coordination via iroh-gossip
//!
//! # Feature Targets
//!
//! - Local push latency: < 10 microseconds
//! - Network push latency: < 100 microseconds
//! - Subscriber receive: < 5 microseconds
//! - At-least-once delivery with acknowledgment
//!
//! # Quick Start
//!
//! ```ignore
//! use ddl::{InMemoryDdl, DDL};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create an in-memory DDL
//!     let ddl = InMemoryDdl::new();
//!
//!     // Subscribe to a topic pattern
//!     let mut stream = ddl.subscribe("metrics.*").await?;
//!
//!     // Push data to a topic
//!     let entry_id = ddl.push("metrics.cpu", b"cpu usage: 42%").await?;
//!
//!     // Receive the entry
//!     if let Some(entry) = stream.next().await? {
//!         println!("Received: {:?}", entry);
//!         // Acknowledge processing
//!         stream.ack(entry.id).await?;
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Backpressure Modes
//!
//! DDL supports multiple backpressure strategies when queues are full:
//!
//! - `DropOldest`: Remove oldest entries to make room
//! - `DropNewest`: Reject new entries
//! - `Block`: Wait until space is available
//! - `Error`: Return an error immediately
//!
//! # KISS Principles
//!
//! This library follows KISS (Keep It Simple, Stupid) principles:
//!
//! - Simple interfaces wrapping powerful backends
//! - Leverage proven libraries (Raft, tokio, TCP)
//! - Comprehensive validation to catch issues early
//! - Consistent syntax across all operations

// Core modules for HPC performance
pub mod ddl;
pub mod ddl_distributed;
pub mod benchmarks;
pub mod topic_queue;
pub mod cluster;
pub mod config;
pub mod constants;
pub mod gossip;
pub mod network;
pub mod node;
pub mod queue;
pub mod traits;
pub mod types;
pub mod wal;
pub mod ddl_wal;

// ============================================================================
// Core DDL Types
// ============================================================================

/// Primary DDL trait for distributed log operations.
///
/// Use this trait when you need type erasure or when implementing custom DDL backends.
pub use crate::traits::ddl::{DDL, DdlConfig, DdlError};

/// In-memory DDL implementation for testing and single-node deployments.
///
/// Zero network overhead, ideal for unit tests and local-only use cases.
pub use crate::ddl::InMemoryDdl;

/// DDL with write-ahead logging for persistence.
///
/// Wraps any DDL implementation with file-based durability.
pub use crate::ddl_wal::DdlWithWal;

// ============================================================================
// Entry Types
// ============================================================================

/// A single entry in the distributed log.
///
/// Contains topic name, payload data, monotonic ID, and metadata.
pub use crate::traits::ddl::Entry;

/// Stream of entries from a subscription.
///
/// Implements async iterator pattern for consuming entries.
pub use crate::traits::ddl::EntryStream;

// ============================================================================
// Configuration Types
// ============================================================================

/// Main configuration for AutoQueues/DDL.
///
/// Contains all settings for queue sizing, backpressure, and network options.
pub use crate::config::Config as AutoQueuesConfig;

/// Queue-specific configuration.
///
/// Defines queue capacity, names, and tuning parameters.
pub use crate::config::QueueConfig;

/// Configuration errors for validation failures.
pub use crate::config::ConfigError;

// ============================================================================
// Queue Types
// ============================================================================

/// Queue error type for operational failures.
pub use crate::queue::source::AutoQueuesError;

/// Trait for queue implementations.
///
/// Defines common operations for all queue types.
pub use crate::traits::queue::QueueTrait;

/// Configuration for interval-based processing.
pub use crate::queue::interval::IntervalConfig;

/// Persistence configuration for durable queues.
pub use crate::queue::persistence::{PersistenceConfig, QueuePersistence};

// ============================================================================
// Network Types
// ============================================================================

/// TCP transport implementation for distributed deployments.
///
/// Uses tokio for async I/O with custom binary protocol.
pub use crate::network::tcp::{TcpTransport, NetworkMessage};

/// Connection information for transport endpoints.
pub use crate::network::transport_traits::ConnectionInfo;

/// Transport trait for pluggable network backends.
pub use crate::network::transport_traits::Transport;

/// Transport-specific error type.
pub use crate::network::transport_traits::TransportError;

/// Transport type enumeration (TCP, RDMA, Hybrid).
pub use crate::network::transport_traits::TransportType;

/// Configuration for hybrid transport mode selection.
pub use crate::network::hybrid::TransportConfig as HybridTransportConfig;

/// ZeroMQ pub/sub broker for message distribution.
pub use crate::network::pubsub::zmq::ZmqPubSubBroker;

/// ZeroMQ pub/sub client for subscribing to topics.
pub use crate::network::pubsub::zmq::ZmqPubSubClient;

// ============================================================================
// Cluster Types
// ============================================================================

/// Raft-based cluster coordination for shard assignment.
pub use crate::cluster::raft_node::{ClusterConfig, RaftNode};

/// Node management and startup utilities.
pub use crate::node::{start_node, AutoQueuesNode};

// ============================================================================
// WAL Types
// ============================================================================

/// Write-ahead log manager for persistence.
///
/// Handles segment rotation, compaction, and recovery.
pub use crate::wal::WalManager;

/// Topic-specific WAL handle.
///
/// Write entries to a specific topic's log.
pub use crate::wal::TopicWal;

/// WAL operation errors.
pub use crate::wal::WalError;

// ============================================================================
// Data Types
// ============================================================================

/// Typed queue data wrapper.
pub use crate::types::QueueData;

/// Queue statistics for monitoring.
pub use crate::types::QueueStats;

/// High-precision timestamp type.
pub use crate::types::Timestamp;