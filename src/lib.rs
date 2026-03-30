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
//! - **DDL** ([`ddl_distributed`]): Distributed log with standalone and distributed modes
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
//! use ddl::{DdlDistributed, DDL, DdlConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a standalone DDL for testing/single-node
//!     let ddl = DdlDistributed::new_standalone(DdlConfig::default());
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
pub mod ddl_distributed;
pub mod benchmarks;
pub mod topic_queue;
pub mod cluster;
pub mod config;
pub mod constants;
pub mod gossip;
pub mod gossip_protocol;
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

/// Dumb Distributed Log implementation.
///
/// Supports two modes:
/// - Standalone: No gossip, owns all topics (for testing/single-node)
/// - Distributed: With gossip for topic ownership discovery (multi-node)
///
/// Use [`DdlDistributed::new_standalone`] for testing and single-node deployments.
/// Use [`DdlDistributed::new_distributed`] for multi-node deployments.
pub use crate::ddl_distributed::DdlDistributed;

/// Type alias for backwards compatibility.
///
/// `InMemoryDdl` is now `DdlDistributed` in standalone mode.
/// Use `DdlDistributed::new_standalone()` for the same behavior.
pub use crate::ddl_distributed::InMemoryDdl;

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

// ============================================================================
// Membership Types (for SCORE integration)
// ============================================================================

/// Membership event for cluster lifecycle tracking.
///
/// Emitted when nodes join, leave, fail, or recover in the cluster.
/// Use with `DdlDistributed::subscribe_membership()` in Raft mode.
///
/// # Example
///
/// ```ignore
/// let mut events = ddl.subscribe_membership()
///     .expect("Raft mode required");
///
/// while let Ok(event) = events.recv().await {
///     println!("[{:?}] {}", event.timestamp, event.event_type);
/// }
/// ```
///
/// See [`MembershipEventType`] for all event variants.
pub use crate::cluster::MembershipEvent;

/// Membership event type enumeration.
///
/// Variants represent the four possible membership changes:
///
/// - [`NodeJoined`] - A new node entered the cluster
/// - [`NodeLeft`] - A node gracefully exited
/// - [`NodeFailed`] - A node crashed or became unreachable
/// - [`NodeRecovered`] - A previously failed node rejoined
///
/// # Example
///
/// ```ignore
/// match event.event_type {
///     MembershipEventType::NodeJoined { node_id, addr } => {
///         println!("Node {} connected from {}", node_id, addr);
///     }
///     MembershipEventType::NodeFailed { node_id } => {
///         eprintln!("Node {} failed - initiating failover", node_id);
///     }
///     _ => {} // Other event types
/// }
/// ```
pub use crate::cluster::MembershipEventType;

/// Current view of cluster membership.
///
/// Provides a snapshot of all known nodes, current leader, and local node ID.
/// Available via `DdlDistributed::membership()` in Raft mode only.
///
/// # Fields
///
/// - `nodes` - HashMap of node_id → [`NodeInfo`]
/// - `leader` - Current Raft leader node_id (None during election)
/// - `local_node_id` - This node's ID
///
/// # Example
///
/// ```ignore
/// if let Some(view) = ddl.membership() {
///     println!("Cluster size: {} nodes", view.nodes.len());
///     println!("Leader: {:?}", view.leader);
/// }
/// ```
pub use crate::cluster::MembershipView;

/// DDL metrics for monitoring and observability.
///
/// Contains operational metrics about the cluster state including
/// active leases, Raft progress, and network statistics.
/// Available via `DdlDistributed::metrics()` in Raft mode only.
///
/// # Metrics Provided
///
/// - **Membership**: `active_leases`, `membership_size`
/// - **State**: `state_size_bytes`, `state_key_count`
/// - **Raft**: `raft_commit_index`, `raft_applied_index`, `raft_leader`
/// - **Network**: `pending_writes`, `pending_reads`
///
/// # Example
///
/// ```ignore
/// if let Some(metrics) = ddl.metrics() {
///     println!("Leases: {}, Membership: {}", metrics.active_leases, metrics.membership_size);
///     println!("Raft: commit={}, applied={}", metrics.raft_commit_index, metrics.raft_applied_index);
///     println!("Pending: writes={}, reads={}", metrics.pending_writes, metrics.pending_reads);
/// }
/// ```
pub use crate::cluster::DdlMetrics;

/// Node information in cluster membership.
///
/// Contains metadata about a single cluster node including
/// network address, last seen timestamp, and leadership status.
///
/// # Fields
///
/// - `node_id` - Unique node identifier
/// - `addr` - Network address (host:port)
/// - `last_seen` - Timestamp of last heartbeat
/// - `is_leader` - Whether this node is the current Raft leader
pub use crate::cluster::NodeInfo;

/// Raft-based cluster node for advanced coordination.
///
/// Provides direct access to Raft cluster operations including
/// topic ownership management and membership subscriptions.
///
/// **Most users should use `DdlDistributed` methods instead.**
/// Direct access is for advanced scenarios requiring:
/// - Multi-node TCP networking
/// - Custom Raft configuration
/// - Low-level cluster operations
///
/// Available via `ddl.raft_cluster.as_ref()` when in Raft mode.
pub use crate::cluster::RaftClusterNode;

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

/// Generate current timestamp in nanoseconds since UNIX epoch.
pub use crate::types::now_nanos;

/// Generate current timestamp in milliseconds since UNIX epoch.
pub use crate::types::now_millis;