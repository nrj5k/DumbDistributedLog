//! Queue Extension Traits Module
//!
//! This module defines extended traits that complement the core Queue trait.
//! These provide specialized functionality for external data sources and advanced features.

/// Trait for external data sources
pub trait ExternalDataSource {
    /// Start receiving external data
    fn start(&mut self) -> Result<(), crate::QueueError>;

    /// Stop receiving external data
    fn stop(&mut self) -> Result<(), crate::QueueError>;

    /// Check if external source is active
    fn is_active(&self) -> bool;

    /// Get connection status
    fn get_connection_status(&self) -> ExternalConnectionStatus;
}

/// Connection status for external data sources
#[derive(Debug, Clone, PartialEq)]
pub enum ExternalConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Trait for ZeroMQ-based data sources
pub trait ZeroMqDataSource<T>: ExternalDataSource {
    /// Configure ZeroMQ endpoint
    fn configure_endpoint(&mut self, endpoint: String) -> Result<(), crate::QueueError>;

    /// Set subscription topic
    fn set_topic(&mut self, topic: String) -> Result<(), crate::QueueError>;

    /// Process received message
    fn process_message(&mut self, message: &[u8]) -> Result<Option<T>, crate::QueueError>;

    /// Get data source metadata
    fn get_metadata(&self) -> serde_json::Value;
}

/// Trait for server-managed queues
pub trait ServerManagedQueue {
    /// Check if server is running
    fn is_server_running(&self) -> bool;

    /// Get server performance metrics
    fn get_server_metrics(&self) -> ServerMetrics;
}

/// Server performance metrics
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    pub uptime_seconds: u64,
    pub processed_items: u64,
    pub current_interval: u64,
    pub average_process_time_ms: f64,
}

/// Trait for hybrid queues (internal + external data)
pub trait HybridQueue<T> {
    /// Get ratio of internal vs external data
    fn get_data_sources_ratio(&self) -> (f64, f64); // (internal, external)

    /// Prefer internal data source
    fn prefer_internal(&mut self) -> Result<(), crate::QueueError>;

    /// Prefer external data source  
    fn prefer_external(&mut self) -> Result<(), crate::QueueError>;

    /// Get last data source used
    fn get_last_data_source(&self) -> DataSourceType;
}

/// Type of data source that provided the last data
#[derive(Debug, Clone, PartialEq)]
pub enum DataSourceType {
    Internal,
    External,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_connection_status() {
        let status = ExternalConnectionStatus::Connected;
        assert_eq!(status, ExternalConnectionStatus::Connected);

        let error_status = ExternalConnectionStatus::Error("Connection failed".to_string());
        assert_eq!(
            error_status,
            ExternalConnectionStatus::Error("Connection failed".to_string())
        );
    }

    #[test]
    fn test_data_source_type() {
        assert_eq!(DataSourceType::Internal, DataSourceType::Internal);
        assert_ne!(DataSourceType::Internal, DataSourceType::External);
    }

    #[test]
    fn test_server_metrics() {
        let metrics = ServerMetrics {
            uptime_seconds: 3600,
            processed_items: 1000,
            current_interval: 500,
            average_process_time_ms: 2.5,
        };

        assert_eq!(metrics.uptime_seconds, 3600);
        assert_eq!(metrics.processed_items, 1000);
        assert_eq!(metrics.current_interval, 500);
        assert_eq!(metrics.average_process_time_ms, 2.5);
    }
}
