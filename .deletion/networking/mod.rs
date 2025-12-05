//! Stratified Networking Module
//!
//! Core transport traits and Quinn implementations for distributed
//! queue networking with separate control and data plane optimizations.

pub mod control_plane;
pub mod data_plane;
pub mod networked_manager;
pub mod quinn_factory;
pub mod transport_traits;

// Re-export core networking types for convenience
pub use self::control_plane::QuinnControlTransport;
pub use self::data_plane::QuinnDataTransport;
pub use self::networked_manager::NetworkedQueueManager;
pub use self::quinn_factory::QuinnTransportFactory;
pub use self::transport_traits::{
    BackpressureLevel, ControlPlaneTransport, DataPlaneTransport, HealthStatus, Transport,
    TransportError, TransportFactory, TransportStats,
};
