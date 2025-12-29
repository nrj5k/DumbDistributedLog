//! Networking module for AutoQueues - HPC-optimized
//!
//! Provides minimal transport abstractions for high-performance communication.
//! Focused on RDMA-ready interfaces for ultra-low latency.

pub mod transport_traits;

pub use transport_traits::{ConnectionInfo, Transport, TransportError, TransportType};