//! Configuration for AutoQueues
//!
//! Provides builder pattern for configuring AutoQueues instances.
//! Supports TOML config file parsing and programmatic configuration.
//!
//! # Config File Format
//!
//! ```toml
//! [node1]
//! host = "127.0.0.1"
//! communication_port = 7067
//! query_port = 7069
//! coordination_port = 7070
//!
//! [node2]
//! host = "127.0.0.1"
//! communication_port = 7167
//! query_port = 7169
//! coordination_port = 7170
//!
//! [local]
//! cpu = "function:cpu"
//! memory = "function:memory"
//!
//! [global.cpu_avg]
//! aggregation = "avg"
//! sources = ["cpu"]
//! interval_ms = 1000
//! ```

use crate::constants;
use crate::queue::persistence::PersistenceConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("Environment variable error: {0}")]
    Env(#[from] std::env::VarError),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Invalid source format: {0}")]
    InvalidSource(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

// ============================================================================
// Source Types
// ============================================================================

/// Source type for local metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    /// System function (cpu, memory, memory_free, disk)
    Function(String),
    /// Expression using local variable names
    Expression(String),
}

impl SourceType {
    /// Parse a source string (e.g., "function:cpu" or "expression:x * 2")
    pub fn parse(s: &str) -> Result<Self, ConfigError> {
        if let Some(name) = s.strip_prefix("function:") {
            Ok(Self::Function(name.to_string()))
        } else if let Some(expr) = s.strip_prefix("expression:") {
            Ok(Self::Expression(expr.to_string()))
        } else {
            // Default to function
            Ok(Self::Function(s.to_string()))
        }
    }

    /// Get the variable name(s) used by this source
    pub fn variables(&self) -> Vec<String> {
        match self {
            Self::Function(name) => vec![name.clone()],
            Self::Expression(expr) => Self::extract_variables(expr),
        }
    }

    /// Extract variable names from an expression
    fn extract_variables(expr: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let mut current = String::new();
        let mut in_var = false;

        for c in expr.chars() {
            if c.is_alphanumeric() || c == '_' {
                current.push(c);
                in_var = true;
            } else if in_var {
                if !current.is_empty() && current != "true" && current != "false" {
                    vars.push(current.clone());
                }
                current.clear();
                in_var = false;
            }
        }

        if in_var && !current.is_empty() {
            vars.push(current);
        }

        vars
    }
}

/// Aggregation type for global metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    #[serde(rename = "avg")]
    Avg,
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "min")]
    Min,
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "stddev")]
    Stddev,
    #[serde(rename = "count")]
    Count(String),
    #[serde(rename = "p50")]
    Percentile50,
    #[serde(rename = "p95")]
    Percentile95,
    #[serde(rename = "p99")]
    Percentile99,
}

impl AggregationType {
    /// Parse aggregation type from string
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        match s {
            "avg" => Ok(Self::Avg),
            "max" => Ok(Self::Max),
            "min" => Ok(Self::Min),
            "sum" => Ok(Self::Sum),
            "stddev" => Ok(Self::Stddev),
            "median" | "p50" => Ok(Self::Percentile50),
            "p95" => Ok(Self::Percentile95),
            "p99" => Ok(Self::Percentile99),
            s if s.starts_with("count:") => {
                let pred = s[6..].to_string();
                Ok(Self::Count(pred))
            }
            _ => Err(ConfigError::InvalidValue(format!(
                "Unknown aggregation type: {}",
                s
            ))),
        }
    }
}

// ============================================================================
// Node Configuration
// ============================================================================

/// Configuration for a single node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub host: String,
    pub communication_port: u16,
    pub query_port: u16,
    pub coordination_port: u16,
    pub pubsub_port: u16,
}

impl NodeConfig {
    /// Get the socket address for node communication
    pub fn communication_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.communication_port)
            .parse()
            .map_err(|e| ConfigError::ParseError(format!("Invalid communication address: {}", e)))
    }

    /// Get the query address
    pub fn query_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.query_port)
            .parse()
            .map_err(|e| ConfigError::ParseError(format!("Invalid query address: {}", e)))
    }

    /// Get the coordination address
    pub fn coordination_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.coordination_port)
            .parse()
            .map_err(|e| ConfigError::ParseError(format!("Invalid coordination address: {}", e)))
    }

    /// Get the pub/sub address for metric distribution
    pub fn pubsub_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.pubsub_port)
            .parse()
            .map_err(|e| ConfigError::ParseError(format!("Invalid pubsub address: {}", e)))
    }
}

/// Raw node table from TOML (table-per-node format)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeTable {
    #[serde(flatten)]
    pub nodes: HashMap<String, NodeConfig>,
}

impl NodeTable {
    /// Collect all configured nodes into a vector
    pub fn collect_nodes(&self) -> Vec<(String, NodeConfig)> {
        self.nodes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

// ============================================================================
// Local and Global Metric Configurations
// ============================================================================

/// Configuration for a local metric source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalMetricConfig {
    pub source: SourceType,
}

impl LocalMetricConfig {
    /// Parse from "function:cpu" or "expression:x" format
    pub fn parse(s: &str) -> Result<Self, ConfigError> {
        Ok(Self {
            source: SourceType::parse(s)?,
        })
    }
}

/// Configuration for a global aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMetricConfig {
    /// Aggregation function (avg, max, min, sum, p95, etc.)
    pub aggregation: String,
    /// Source queue names to aggregate
    pub sources: Vec<String>,
    /// How often to compute in milliseconds
    #[serde(default = "global_default_interval")]
    pub interval_ms: u64,
    /// Expression for expression-based aggregation
    #[serde(default)]
    pub expression: Option<String>,
}

fn global_default_interval() -> u64 {
    constants::config::DEFAULT_INTERVAL_MS
}

/// Debug configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub simulate_sensors: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            simulate_sensors: false,
        }
    }
}

// ============================================================================
// Main Config Structure
// ============================================================================

/// Main configuration structure for AutoQueues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node table (table-per-node format)
    #[serde(default)]
    pub nodes: NodeTable,

    /// Local metric sources
    #[serde(default)]
    pub local: HashMap<String, String>,

    /// Global aggregations
    #[serde(default)]
    pub global: HashMap<String, GlobalMetricConfig>,

    /// Queue settings
    #[serde(default)]
    pub queue: QueueConfig,

    /// Debug settings
    #[serde(default)]
    pub debug: DebugConfig,

    /// Persistence settings
    #[serde(default)]
    pub persistence: Option<PersistenceConfig>,
}

/// Queue-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    #[serde(default = "queue_default_capacity")]
    pub capacity: usize,
    #[serde(default = "queue_default_interval")]
    pub default_interval: u64,
}

fn queue_default_capacity() -> usize {
    constants::config::DEFAULT_CAPACITY
}

fn queue_default_interval() -> u64 {
    constants::config::DEFAULT_INTERVAL_MS
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            capacity: queue_default_capacity(),
            default_interval: queue_default_interval(),
        }
    }
}

// ============================================================================
// Builder Pattern
// ============================================================================

/// Builder for Config
#[derive(Debug)]
pub struct ConfigBuilder {
    config: Config,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            config: Config::default(),
        }
    }
}

impl ConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node
    pub fn add_node(
        mut self,
        name: &str,
        host: &str,
        communication_port: u16,
        query_port: u16,
        coordination_port: u16,
        pubsub_port: u16,
    ) -> Self {
        let node_config = NodeConfig {
            host: host.to_string(),
            communication_port,
            query_port,
            coordination_port,
            pubsub_port,
        };

        self.config
            .nodes
            .nodes
            .insert(name.to_string(), node_config);

        self
    }

    /// Add a local metric source
    pub fn add_local_metric(mut self, name: &str, source: &str) -> Self {
        self.config
            .local
            .insert(name.to_string(), source.to_string());
        self
    }

    /// Add a global aggregation
    pub fn add_global_aggregation(
        mut self,
        name: &str,
        aggregation: &str,
        sources: &[&str],
        interval_ms: u64,
    ) -> Self {
        self.config.global.insert(
            name.to_string(),
            GlobalMetricConfig {
                aggregation: aggregation.to_string(),
                sources: sources.iter().map(|s| s.to_string()).collect(),
                interval_ms,
                expression: None,
            },
        );
        self
    }

    /// Add an expression-based aggregation
    pub fn add_expression_aggregation(
        mut self,
        name: &str,
        expression: &str,
        aggregation: &str,
        sources: &[&str],
        interval_ms: u64,
    ) -> Self {
        self.config.global.insert(
            name.to_string(),
            GlobalMetricConfig {
                aggregation: aggregation.to_string(),
                sources: sources.iter().map(|s| s.to_string()).collect(),
                interval_ms,
                expression: Some(expression.to_string()),
            },
        );
        self
    }

    /// Set queue capacity
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.config.queue.capacity = capacity;
        self
    }

    /// Set default interval
    pub fn default_interval(mut self, interval_ms: u64) -> Self {
        self.config.queue.default_interval = interval_ms;
        self
    }

    /// Enable sensor simulation
    pub fn simulate_sensors(mut self, enabled: bool) -> Self {
        self.config.debug.simulate_sensors = enabled;
        self
    }

    /// Enable verbose mode
    pub fn verbose(mut self, enabled: bool) -> Self {
        self.config.debug.verbose = enabled;
        self
    }

    /// Set persistence configuration
    pub fn persistence(mut self, config: PersistenceConfig) -> Self {
        self.config.persistence = Some(config);
        self
    }

    /// Build the configuration
    pub fn build(self) -> Config {
        self.config
    }
}

impl Config {
    /// Create a new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Load configuration from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get my node configuration based on NODE_ID environment variable
    pub fn my_node(&self, node_id: u64) -> Result<NodeConfig, ConfigError> {
        let node_name = format!("node{}", node_id);
        self.nodes
            .nodes
            .get(&node_name)
            .cloned()
            .ok_or_else(|| ConfigError::NodeNotFound(node_name))
    }

    /// Get all other nodes (excluding self)
    pub fn other_nodes(&self, node_id: u64) -> Vec<(String, NodeConfig)> {
        self.nodes
            .collect_nodes()
            .into_iter()
            .filter(|(name, _)| name != &format!("node{}", node_id))
            .collect()
    }

    /// Get all nodes
    pub fn all_nodes(&self) -> Vec<(String, NodeConfig)> {
        self.nodes.collect_nodes()
    }

    /// Detect node ID from environment or hostname
    pub fn detect_node_id(&self) -> Result<u64, ConfigError> {
        // First check NODE_ID environment variable
        if let Ok(id) = env::var("NODE_ID") {
            return id
                .parse()
                .map_err(|_| ConfigError::InvalidValue("Invalid NODE_ID".to_string()));
        }

        // Otherwise use hostname lookup
        let hostname = env::var("HOSTNAME")
            .or_else(|_| env::var("HOST"))
            .map_err(|_| ConfigError::InvalidValue("Cannot determine hostname".to_string()))?;

        for (i, (name, _)) in self.all_nodes().iter().enumerate() {
            // Simple case-insensitive match
            if name.to_lowercase() == hostname.to_lowercase() {
                return Ok((i + 1) as u64);
            }
            // Also check if hostname matches the host field
            if let Some(config) = self.my_node((i + 1) as u64).ok() {
                if config.host == hostname {
                    return Ok((i + 1) as u64);
                }
            }
        }

        Err(ConfigError::InvalidValue(format!(
            "Hostname '{}' not found in node list",
            hostname
        )))
    }

    /// Get local metric source
    pub fn local_source(&self, name: &str) -> Option<SourceType> {
        self.local.get(name).map(|s| SourceType::parse(s).unwrap())
    }

    /// Get global metric configuration
    pub fn global_metric(&self, name: &str) -> Option<&GlobalMetricConfig> {
        self.global.get(name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nodes: NodeTable::default(),
            local: HashMap::new(),
            global: HashMap::new(),
            queue: QueueConfig::default(),
            debug: DebugConfig::default(),
            persistence: None,
        }
    }
}

// ============================================================================
// Config Generator
// ============================================================================

/// Helper to generate example configurations
pub struct ConfigGenerator;

impl ConfigGenerator {
    /// Generate a local test config with 3 nodes on localhost
    pub fn local_test(num_nodes: u64, base_port: u16) -> Config {
        let mut config = Config::new();
        let port_offset = 100;

        for i in 1..=num_nodes {
            let name = format!("node{}", i);
            let port = base_port + (i as u16 * port_offset);

            config.nodes.nodes.insert(
                name,
                NodeConfig {
                    host: "127.0.0.1".to_string(),
                    communication_port: port,
                    query_port: port + 2,
                    coordination_port: port + 3,
                    pubsub_port: port + 4,
                },
            );
        }

        // Add common local metrics
        config
            .local
            .insert("cpu".to_string(), "function:cpu".to_string());
        config
            .local
            .insert("memory".to_string(), "function:memory".to_string());
        config.local.insert(
            "memory_free".to_string(),
            "function:memory_free".to_string(),
        );
        config.local.insert(
            "temperature".to_string(),
            "function:sensor_temp".to_string(),
        );

        // Add global aggregations
        config.global.insert(
            "cpu_avg".to_string(),
            GlobalMetricConfig {
                aggregation: "avg".to_string(),
                sources: vec!["cpu".to_string()],
                interval_ms: 1000,
                expression: None,
            },
        );

        config.global.insert(
            "temperature_max".to_string(),
            GlobalMetricConfig {
                aggregation: "max".to_string(),
                sources: vec!["temperature".to_string()],
                interval_ms: 2000,
                expression: None,
            },
        );

        config.global.insert(
            "node_health_score".to_string(),
            GlobalMetricConfig {
                aggregation: "avg".to_string(),
                sources: vec!["cpu".to_string(), "memory_free".to_string()],
                interval_ms: 1000,
                expression: Some("(100 - cpu) + memory_free".to_string()),
            },
        );

        // Debug settings for testing
        config.debug.simulate_sensors = true;
        config.debug.verbose = true;

        config
    }

    /// Generate a production config with the given nodes
    pub fn production(nodes: &[(&str, &str)]) -> Config
    where
        for<'a> &'a str: std::fmt::Display,
    {
        let mut config = Config::new();

        for (i, (name, host)) in nodes.iter().enumerate() {
            let port = 6967 + (i as u16 * 100);

            config.nodes.nodes.insert(
                name.to_string(),
                NodeConfig {
                    host: host.to_string(),
                    communication_port: port,
                    query_port: port + 2,
                    coordination_port: port + 3,
                    pubsub_port: port + 4,
                },
            );
        }

        // Add default local metrics
        config
            .local
            .insert("cpu".to_string(), "function:cpu".to_string());
        config
            .local
            .insert("memory".to_string(), "function:memory".to_string());
        config.local.insert(
            "memory_free".to_string(),
            "function:memory_free".to_string(),
        );

        config
    }
}
