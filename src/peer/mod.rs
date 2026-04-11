//! Peer-to-peer request/response module for DDL.
//!
//! Provides distributed peer communication with request correlation,
//! health monitoring, and circuit breaker patterns.

pub mod correlation;
pub mod error;
pub mod response;
pub mod types;

pub mod advertisement;
pub mod cache;
pub mod circuit_breaker;
pub mod codec;
pub mod config;
pub mod fetcher;
pub mod handler;
pub mod health;
pub mod pending;
pub mod registry;
pub mod request;
pub mod routing;

// Re-exports for convenience
pub use advertisement::ServiceAdvertisement;
pub use cache::PeerCache;
pub use circuit_breaker::CircuitBreaker;
pub use config::PeerConfig;
pub use error::PeerError;
pub use fetcher::PeerFetcher;
pub use handler::PeerRequestHandler;
pub use health::HealthMonitor;
pub use registry::PeerRegistry;
pub use request::PeerRequest;
pub use response::PeerResponse;
pub use routing::PeerRouter;
pub use types::{now_millis, MetricKey, RequestId};
