//! AutoQueues Performance Constants
//!
//! This module consolidates all magic numbers to make performance tuning easier.
//! Changes here affect the entire system - test thoroughly after modifications.

pub mod time {
    //! Timing-related constants
    //!
    //! INCREASING these values:
    //! - Increases latency in responsiveness
    //! - Reduces CPU overhead from frequent operations
    //! - May cause missed data points in fast-changing metrics
    //!
    //! DECREASING these values:
    //! - Improves responsiveness to changes
    //! - Increases CPU overhead and context switches
    //! - May cause lock contention under load

    /// Interval for metric collection in milliseconds
    /// Default: 1000ms (1 second)
    pub const METRICS_INTERVAL_MS: u64 = 1000;

    /// Interval for health check in milliseconds  
    /// Default: 5000ms (5 seconds)
    pub const HEALTH_CHECK_INTERVAL_MS: u64 = 5000;

    /// Connection timeout in milliseconds
    /// Default: 30000ms (30 seconds)
    pub const CONNECTION_TIMEOUT_MS: u64 = 30_000;

    /// Retry backoff base delay in milliseconds
    /// Default: 100ms
    ///
    /// INCREASING: More resilient to network issues, slower recovery
    /// DECREASING: Faster recovery, may overwhelm network under issues
    pub const RETRY_BASE_DELAY_MS: u64 = 100;

    /// Maximum retry attempts before giving up
    /// Default: 3
    ///
    /// INCREASING: More resilient, longer failure cascades
    /// DECREASING: Faster failure detection, less resilient
    pub const MAX_RETRIES: usize = 3;

    /// Exponential backoff multiplier
    /// Default: 2 (doubles each retry)
    ///
    /// INCREASING: Slower recovery, more resilient to thundering herd
    /// DECREASING: Faster recovery, may overwhelm network
    pub const RETRY_BACKOFF_MULTIPLIER: u64 = 2;

    /// Interval for aggregation in milliseconds
    /// Default: 500ms
    pub const AGGREGATION_INTERVAL_MS: u64 = 500;

    /// Short sleep interval for polling operations
    /// Default: 100ms
    pub const SHORT_SLEEP_INTERVAL_MS: u64 = 100;

    /// Very short sleep interval for fast polling
    /// Default: 10ms
    pub const VERY_SHORT_SLEEP_INTERVAL_MS: u64 = 10;

    /// Election timeout in milliseconds
    /// Default: 1000ms
    pub const ELECTION_TIMEOUT_MS: u64 = 1000;

    /// Tick interval for Raft consensus
    /// Default: 100ms
    pub const TICK_INTERVAL_MS: u64 = 100;

    /// Query timeout in milliseconds
    /// Default: 1000ms
    pub const QUERY_TIMEOUT_MS: u64 = 1000;

    /// Freshness timeout (2 × AIMD_max)
    /// Default: 10000ms (10 seconds)
    pub const FRESHNESS_TIMEOUT_MS: u64 = 10_000;

    /// Check interval for leader checks
    /// Default: 1000ms
    pub const LEADER_CHECK_INTERVAL_MS: u64 = 1000;

    /// Minimum interval for AIMD algorithm
    /// Default: 100ms
    pub const AIMD_MIN_INTERVAL_MS: u64 = 100;

    /// Maximum interval for AIMD algorithm
    /// Default: 5000ms
    pub const AIMD_MAX_INTERVAL_MS: u64 = 5_000;
}

pub mod memory {
    //! Memory allocation constants
    //!
    //! INCREASING these values:
    //! - Reduces reallocation overhead
    //! - Increases baseline memory usage
    //! - May waste memory for small workloads
    //!
    //! DECREASING these values:
    //! - Saves memory for small workloads
    //! - Increases reallocation frequency
    //! - May cause allocation spikes under load

    /// Default HashMap capacity for variable storage
    /// Default: 8
    pub const HASHMAP_DEFAULT_CAPACITY: usize = 8;

    /// Default Vec capacity for queue buffers
    /// Default: 32
    pub const VEC_DEFAULT_CAPACITY: usize = 32;

    /// Maximum queue depth before backpressure
    /// Default: 1000
    ///
    /// INCREASING: More buffering, higher memory usage
    /// DECREASING: Earlier backpressure, lower memory usage
    pub const MAX_QUEUE_DEPTH: usize = 1024;

    /// Channel buffer size for metrics
    /// Default: 100
    pub const METRICS_CHANNEL_BUFFER_SIZE: usize = 100;

    /// Small channel buffer size
    /// Default: 1
    pub const SMALL_CHANNEL_BUFFER_SIZE: usize = 1;

    /// History buffer size for entries
    /// Default: 100
    pub const HISTORY_BUFFER_SIZE: usize = 100;

    /// Small history buffer size
    /// Default: 10
    pub const SMALL_HISTORY_BUFFER_SIZE: usize = 10;
}

pub mod network {
    //! Network-related constants
    //!
    //! INCREASING these values:
    //! - Better handling of burst traffic
    //! - Higher memory usage for buffers
    //! - May mask network issues longer
    //!
    //! DECREASING these values:
    //! - Lower memory footprint
    //! - Earlier detection of network issues
    //! - May drop bursts under load

    /// ZMQ LINGER value for socket cleanup
    /// Default: 0 (immediate close)
    pub const ZMQ_LINGER: i32 = 0;

    /// Maximum message size in bytes (1MB)
    /// Default: 1_048_576
    pub const MAX_MESSAGE_SIZE: usize = 1_048_576;

    /// Heartbeat interval in milliseconds
    /// Default: 1000ms
    pub const HEARTBEAT_INTERVAL_MS: u64 = 1000;

    /// Peer discovery timeout
    /// Default: 5000ms
    pub const PEER_DISCOVERY_TIMEOUT_MS: u64 = 5_000;

    /// Default data plane port
    /// Default: 6966
    pub const DEFAULT_DATA_PORT: u16 = 6966;

    /// Default coordination port
    /// Default: 6968
    pub const DEFAULT_COORDINATION_PORT: u16 = 6968;

    /// Default query port
    /// Default: 6969
    pub const DEFAULT_QUERY_PORT: u16 = 6969;

    /// Default QUIC port
    /// Default: 6967
    pub const DEFAULT_QUIC_PORT: u16 = 6967;

    /// Receive timeout for ZMQ sockets
    /// Default: 1000ms
    pub const ZMQ_RCVTIMEO_MS: i32 = 1000;

    /// Short receive timeout for ZMQ sockets
    /// Default: 100ms
    pub const ZMQ_SHORT_RCVTIMEO_MS: i32 = 100;
}

pub mod expression {
    //! Expression evaluation constants
    //!
    //! INCREASING these values:
    //! - Allows more complex expressions
    //! - May increase expression parsing time
    //! - Higher memory usage for compiled expressions
    //!
    //! DECREASING these values:
    //! - Prevents complex/malicious expressions
    //! - Faster validation
    //! - May reject valid complex expressions

    /// Maximum expression length in characters
    /// Default: 1000
    pub const MAX_EXPRESSION_LENGTH: usize = 1000;

    /// Maximum number of variables in expression
    /// Default: 20
    pub const MAX_EXPRESSION_VARS: usize = 20;

    /// Expression evaluation timeout in milliseconds
    /// Default: 100ms
    pub const EXPRESSION_TIMEOUT_MS: u64 = 100;
}

pub mod lock {
    //! Lock and concurrency constants
    //!
    //! INCREASING these values:
    //! - More tolerant of slow operations under lock
    //! - May cause head-of-line blocking
    //! - Higher latency under contention
    //!
    //! DECREASING these values:
    //! - More responsive under contention
    //! - May cause lock timeout errors
    //! - Better for fast operations

    /// Lock timeout for read operations
    /// Default: 5000ms
    pub const READ_LOCK_TIMEOUT_MS: u64 = 5_000;

    /// Lock timeout for write operations
    /// Default: 1000ms
    pub const WRITE_LOCK_TIMEOUT_MS: u64 = 1_000;

    /// Maximum pending lock waiters
    /// Default: 16
    pub const MAX_LOCK_WAITERS: usize = 16;
}

pub mod system {
    //! System-level constants

    /// Maximum number of queues
    /// Default: 100
    pub const MAX_QUEUES: usize = 100;

    /// Graceful shutdown timeout in milliseconds
    /// Default: 5000ms
    pub const SHUTDOWN_TIMEOUT_MS: u64 = 5_000;

    /// Base port offset for additional publishers
    /// Default: 100
    pub const BASE_PORT_OFFSET: u16 = 100;

    /// Consensus count for leader queries
    /// Default: 3
    pub const CONSENSUS_COUNT: usize = 3;
}
