//! Unified Configuration System - KISS Simplified

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    FileNotFound(String),
    ParseError(String),
    ValidationError(String),
    SectionNotFound(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "Config file not found: {}", path),
            ConfigError::ParseError(msg) => write!(f, "TOML parse error: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Config validation error: {}", msg),
            ConfigError::SectionNotFound(section) => write!(f, "Config section not found: {}", section),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::FileNotFound(err.to_string())
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::ParseError(err.to_string())
    }
}

/// Main configuration structure - EXTENDED with AutoQueue coordination
#[derive(Debug, Deserialize, Serialize)]
pub struct QueueConfig {
    pub queues: QueuesSection,
    /// NEW: AutoQueue coordination sections for distributed cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoqueues: Option<AutoQueuesConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueuesSection {
    pub base: Vec<String>,
    pub derived: HashMap<String, DerivedFormula>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedFormula {
    pub formula: String,
}

/// NEW: AutoQueue distributed coordination configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct AutoQueuesConfig {
    /// Cluster node addresses for distributed operation
    /// Format: "hostname:port" e.g., ["server1:6966", "server2:6966", "server3:6966"]
    #[serde(default)]
    pub cluster_nodes: Vec<String>,
    /// Node assignment strategy - automatic vs manual control
    #[serde(default)]
    pub assignment_strategy: AssignmentStrategy,
    /// Auto-assignment settings (default: true)
    #[serde(default = "default_true")]
    pub auto_assignment: bool,
    /// Auto-discovery settings
    #[serde(default = "default_true")]
    pub auto_discovery: bool,
    /// Derived metrics configuration (aggregations of local metrics)
    #[serde(default)]
    pub derived_metrics: HashMap<String, DerivedMetricConfig>,
    /// Node scores for weighted leader assignment
    #[serde(default)]
    pub node_scores: HashMap<String, f64>,  // hostname → score
    /// Node order for hostname → node_id mapping
    #[serde(default)]
    pub node_order: Vec<String>,  // hostnames in order
    /// Per-metric leader assignment (optional override)
    #[serde(default)]
    pub leaders: HashMap<u64, LeaderGroupConfig>,
    /// Bootstrap and discovery configuration
    pub bootstrap: BootstrapConfig,
    /// Node mapping customization (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_mapping: Option<NodeMappingConfig>,
}

/// NEW: Assignment strategy for node-to-coordinator mapping
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
pub enum AssignmentStrategy {
    #[default]
    Automatic,           // System handles everything
    HostnameMapping,     // User provides specific mappings
    TagBased,           // Map by node capabilities/roles
    LocationBased,      // Map by physical location
    RoundRobin,         // Simple distribution
}

/// NEW: Derived metric configuration (aggregations of local metrics)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DerivedMetricConfig {
    pub aggregation: String,           // sum, average, max, min
    pub sources: Vec<String>,          // local metric sources
    #[serde(default = "default_true")]
    pub auto_aggregate: bool,          // Enable automatic calculation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_nodes: Option<usize>, // Minimum nodes for reliable aggregate
}

/// NEW: Per-metric leader configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct LeaderGroupConfig {
    pub node_group: Vec<u64>,           // Logical node IDs for this coordinator
    pub leader_metrics: Vec<String>,    // Metrics this coordinator calculates
    pub election_timeout_ms: u32,
    pub heartbeat_interval_ms: u32,
}

    /// NEW: Bootstrap and discovery configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BootstrapConfig {
    pub anchor_nodes: Vec<u64>,         // Logical node IDs for anchor points
    pub discovery_port: u16,
    pub autoqueues_port: u16,           // Standard AutoQueues coordination port
    #[serde(default = "default_coordination_port")]
    pub coordination_port: u16,         // Separate port for Raft coordination (default: 6968)
    #[serde(default = "default_data_port")]
    pub data_port: u16,                 // Data plane port (default: 6966)
    #[serde(default = "default_query_port")]
    pub query_port: u16,                // Leader REQ/REP port (default: 6969)
    #[serde(default = "default_aimd_min_interval")]
    pub aimd_min_interval_ms: u64,      // AIMD minimum interval in ms
    #[serde(default = "default_aimd_max_interval")]
    pub aimd_max_interval_ms: u64,      // AIMD maximum interval in ms (for freshness timeout)
    #[serde(default = "default_true")]
    pub enable_multicast: bool,         // UDP multicast discovery
}

fn default_coordination_port() -> u16 {
    6968  // One port higher than data port
}

fn default_data_port() -> u16 {
    6966  // Default data plane port
}

fn default_query_port() -> u16 {
    6969  // Default query port for REQ/REP
}

fn default_aimd_min_interval() -> u64 {
    100  // 100ms minimum interval
}

fn default_aimd_max_interval() -> u64 {
    5000  // 5000ms maximum interval (for freshness timeout detection)
}

/// NEW: Node mapping customization (hostnames → logical IDs)
#[derive(Debug, Deserialize, Serialize)]
pub struct NodeMappingConfig {
    pub mappings: HashMap<String, u64>, // hostname → logical_id mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_mappings: Option<HashMap<String, LocationInfo>>,
}

/// NEW: Physical location information for location-based assignment
#[derive(Debug, Deserialize, Serialize)]
pub struct LocationInfo {
    pub rack: String,
    pub datacenter: String,
    pub zone: String,
}

/// NEW: Node tagging for capability-based assignment
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct NodeTags {
    pub capabilities: Vec<String>,      // ["cpu_heavy", "memory_heavy", "storage"]
    pub role: Option<String>,          // ["worker", "compute", "storage", "manager"]
    pub workload_type: Option<String>, // ["batch", "interactive", "ml_training"]
}

impl QueueConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config: QueueConfig = toml::from_str(&content)?;
        
        // NEW: Validate AutoQueue config if present
        if let Some(ref aq_config) = config.autoqueues {
            Self::validate_autoqueue_config(aq_config)?;
        }
        
        Ok(config)
    }

    /// Create configuration from TOML string
    pub fn from_str(toml_content: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(toml_content)?)
    }

    /// Get base metrics
    pub fn get_base_metrics(&self) -> &[String] {
        &self.queues.base
    }

    /// Get derived formulas
    pub fn get_derived_formulas(&self) -> impl Iterator<Item = (&str, &str)> {
        self.queues
            .derived
            .iter()
            .map(|(name, formula)| (name.as_str(), formula.formula.as_str()))
    }

    /// Get specific derived formula
    pub fn get_derived_formula(&self, name: &str) -> Option<&str> {
        self.queues
            .derived
            .get(name)
            .map(|formula| formula.formula.as_str())
    }

    /// NEW: Get AutoQueue coordination config
    pub fn get_autoqueue_config(&self) -> Option<&AutoQueuesConfig> {
        self.autoqueues.as_ref()
    }

    /// NEW: Check if auto-assignment is enabled
    pub fn is_auto_assignment_enabled(&self) -> bool {
        self.autoqueues.as_ref().map(|aq| aq.auto_assignment).unwrap_or(true)
    }

    /// NEW: Get assignment strategy
    pub fn get_assignment_strategy(&self) -> AssignmentStrategy {
        self.autoqueues.as_ref().map(|aq| aq.assignment_strategy.clone()).unwrap_or_default()
    }

    /// NEW: Get derived metrics configuration
    pub fn get_derived_metrics(&self) -> HashMap<String, DerivedMetricConfig> {
        self.autoqueues.as_ref()
            .map(|aq| aq.derived_metrics.clone())
            .unwrap_or_default()
    }

    /// NEW: Get derived metric by name
    pub fn get_derived_metric(&self, name: &str) -> Option<&DerivedMetricConfig> {
        self.autoqueues.as_ref()
            .and_then(|aq| aq.derived_metrics.get(name))
    }

    /// NEW: Get node scores (hostname → score)
    pub fn get_node_scores(&self) -> HashMap<String, f64> {
        self.autoqueues.as_ref()
            .map(|aq| aq.node_scores.clone())
            .unwrap_or_default()
    }

    /// NEW: Get node order (for hostname → node_id mapping)
    pub fn get_node_order(&self) -> &[String] {
        self.autoqueues.as_ref()
            .map(|aq| aq.node_order.as_slice())
            .unwrap_or(&[])
    }

    /// NEW: Convert hostname to node_id using node_order
    /// Returns 0 if hostname not found
    pub fn hostname_to_node_id(&self, hostname: &str) -> u64 {
        self.get_node_order().iter()
            .position(|h| h == hostname)
            .map(|i| (i + 1) as u64)  // 1-indexed
            .unwrap_or(0)  // 0 = not found
    }

    /// NEW: Get node_id for this node (from hostname in node_order)
    pub fn get_local_node_id(&self) -> u64 {
        // Get hostname from system
        if let Ok(hostname) = std::env::var("HOSTNAME") {
            self.hostname_to_node_id(&hostname)
        } else {
            // Fallback: use first node in order as this node
            1
        }
    }

    /// NEW: Get AIMD intervals from bootstrap config
    pub fn get_aimd_intervals(&self) -> (u64, u64) {
        if let Some(aq) = self.autoqueues.as_ref() {
            (aq.bootstrap.aimd_min_interval_ms, aq.bootstrap.aimd_max_interval_ms)
        } else {
            (100, 5000)  // defaults
        }
    }

    /// NEW: Get freshness timeout (2 × AIMD max interval)
    pub fn get_freshness_timeout_ms(&self) -> u64 {
        let (_, max_interval) = self.get_aimd_intervals();
        max_interval * 2
    }

    /// NEW: Get coordination port
    pub fn get_coordination_port(&self) -> u16 {
        self.autoqueues.as_ref()
            .map(|aq| aq.bootstrap.coordination_port)
            .unwrap_or(6968)
    }

    /// NEW: Get data port
    pub fn get_data_port(&self) -> u16 {
        self.autoqueues.as_ref()
            .map(|aq| aq.bootstrap.data_port)
            .unwrap_or(6967)
    }

    /// NEW: Get cluster nodes for distributed operation
    pub fn get_cluster_nodes(&self) -> &[String] {
        self.autoqueues.as_ref()
            .map(|aq| aq.cluster_nodes.as_slice())
            .unwrap_or(&[])
    }

    /// NEW: Check if running in distributed mode
    pub fn is_distributed(&self) -> bool {
        !self.get_cluster_nodes().is_empty()
    }

    /// NEW: Get query port (for leader REQ/REP)
    pub fn get_query_port(&self) -> u16 {
        self.autoqueues.as_ref()
            .map(|aq| aq.bootstrap.query_port)
            .unwrap_or(6969)
    }

    /// NEW: Validate AutoQueue configuration
    fn validate_autoqueue_config(aq_config: &AutoQueuesConfig) -> Result<(), ConfigError> {
        // Validate logical ID uniqueness in node groups
        let mut all_node_ids = HashSet::new();
        for leader_config in aq_config.leaders.values() {
            for node_id in &leader_config.node_group {
                if !all_node_ids.insert(node_id) {
                    return Err(ConfigError::ValidationError(format!(
                        "Node {} appears in multiple coordinator groups", node_id
                    )));
                }
            }
        }

        // Validate derived metrics
        for (metric_name, metric_config) in &aq_config.derived_metrics {
            match metric_config.aggregation.as_str() {
                "sum" | "average" | "max" | "min" => {},
                _ => return Err(ConfigError::ValidationError(format!(
                    "Invalid aggregation type '{}' for metric {}",
                    metric_config.aggregation, metric_name
                ))),
            }

            // Validate sources reference local metrics
            for source in &metric_config.sources {
                if !source.starts_with("local.") {
                    return Err(ConfigError::ValidationError(format!(
                        "Derived metric '{}' source '{}' must start with 'local.'",
                        metric_name, source
                    )));
                }
            }
        }

        // Validate node_scores hostnames match node_order
        if !aq_config.node_order.is_empty() {
            for hostname in aq_config.node_scores.keys() {
                if !aq_config.node_order.contains(hostname) {
                    return Err(ConfigError::ValidationError(format!(
                        "Node score hostname '{}' not found in node_order",
                        hostname
                    )));
                }
            }
        }

        Ok(())
    }

    /// Create standard configuration (extended)
    pub fn create_standard() -> Self {
        let toml_str = r#"
[queues]
base = ["cpu_percent", "memory_percent", "drive_percent"]

[queues.derived.health_score]
formula = "(local.cpu_percent + local.memory_percent + local.drive_percent) / 3.0"

[queues.derived.weighted_health]
formula = "(local.cpu_percent * 0.4) + (local.memory_percent * 0.3) + (local.drive_percent * 0.3)"

[autoqueues]
# Simple distributed coordination with auto-assignment
assignment_strategy = "Automatic"
auto_assignment = true
auto_discovery = true

[autoqueues.bootstrap]
anchor_nodes = [1, 2, 3, 4]  # Bootstrap with first 4 logical nodes
discovery_port = 5353
autoqueues_port = 6942

[autoqueues.global.cpu_usage]
aggregation = "average"
auto_aggregate = true
required_nodes = 4

[autoqueues.global.memory_usage]
aggregation = "sum"
auto_aggregate = true
required_nodes = 4

[autoqueues.leaders.1]
node_group = [1, 2, 3, 4] # Logical node IDs
leader_metrics = ["cpu_usage", "memory_usage"]
election_timeout_ms = 200
heartbeat_interval_ms = 50
"#;
        Self::from_str(toml_str).expect("Standard AutoQueue config should be valid")
    }

    /// Create minimal configuration
    pub fn create_minimal() -> Self {
        let toml_str = r#"
[queues]
base = ["cpu_percent", "memory_percent"]

[queues.derived.simple_average]
formula = "(local.cpu_percent + local.memory_percent) / 2.0"

[autoqueues]
# Ultra-minimal distributed config
auto_assignment = true
auto_discovery = true

[autoqueues.bootstrap]
anchor_nodes = [1, 2]
discovery_port = 5353
autoqueues_port = 6942
"#;
        Self::from_str(toml_str).expect("Minimal AutoQueue config should be valid")
    }
}

// Suppress dead code warning
#[allow(dead_code)]
const fn default_true() -> bool { true }