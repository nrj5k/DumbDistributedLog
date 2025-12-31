//! Configuration for AutoQueues
//!
//! Provides a builder pattern for configuring AutoQueues instances.

use std::env;
use std::fs;
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Environment variable error: {0}")]
    Env(#[from] std::env::VarError),
    
    #[error("Invalid namespace: {0}")]
    InvalidNamespace(String),
}

/// Discovery method for cluster nodes
#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
    Static,
    Mdns,
}

/// Configuration for AutoQueues
#[derive(Debug, Clone)]
pub struct Config {
    pub nodes: Vec<String>,
    pub discovery: DiscoveryMethod,
    pub namespace: Option<String>,
    pub capacity: usize,
    pub default_interval: u64,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            discovery: DiscoveryMethod::Static,
            namespace: None,
            capacity: 1024,
            default_interval: 1000,
        }
    }
    
    /// Set nodes for the configuration
    pub fn nodes(mut self, nodes: &[&str]) -> Self {
        self.nodes = nodes.iter().map(|s| s.to_string()).collect();
        self
    }
    
    /// Load nodes from a file
    pub fn nodes_from_file(mut self, path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        self.nodes = content.lines().map(|s| s.trim().to_string()).collect();
        Ok(self)
    }
    
    /// Load nodes from an environment variable
    pub fn nodes_from_env(mut self, key: &str) -> Result<Self, ConfigError> {
        let content = env::var(key)?;
        self.nodes = content.split(',').map(|s| s.trim().to_string()).collect();
        Ok(self)
    }
    
    /// Set the discovery method
    pub fn discovery(mut self, method: DiscoveryMethod) -> Self {
        self.discovery = method;
        self
    }
    
    /// Set the namespace
    pub fn namespace(mut self, ns: &str) -> Result<Self, ConfigError> {
        if ns.is_empty() {
            return Err(ConfigError::InvalidNamespace("Namespace cannot be empty".to_string()));
        }
        self.namespace = Some(ns.to_string());
        Ok(self)
    }
    
    /// Set the queue capacity
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = n;
        self
    }
    
    /// Set the default interval in milliseconds
    pub fn default_interval(mut self, ms: u64) -> Self {
        self.default_interval = ms;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}