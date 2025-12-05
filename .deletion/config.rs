//! Configuration trait definitions for AutoQueues
//!
//! Provides unified configuration interface following KISS principle.
//! Ultra-minimal trait with 3 methods for maximum flexibility.

use std::collections::HashMap;

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    FileNotFound(String),
    ParseError(String),
    ValidationError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "Config file not found: {}", path),
            ConfigError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Ultra-minimal configuration trait for maximum flexibility
pub trait Configuration: Send + Sync {
    /// Get queue configurations
    fn queue_configs(&self) -> &HashMap<String, crate::QueueConfig>;
    
    /// Get global configuration
    fn global_config(&self) -> &crate::GlobalConfig;
    
    /// Load configuration from file
    fn from_file(path: &str) -> Result<Self, ConfigError> where Self: Sized;
}