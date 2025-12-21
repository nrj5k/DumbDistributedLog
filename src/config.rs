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
    /// Node assignment strategy - automatic vs manual control
    #[serde(default)]
    pub assignment_strategy: AssignmentStrategy,
    /// Auto-assignment settings (default: true)
    #[serde(default = "default_true")]
    pub auto_assignment: bool,
    /// Auto-discovery settings
    #[serde(default = "default_true")]
    pub auto_discovery: bool,
    /// Global metrics configuration (auto-aggregations)
    #[serde(default)]
    pub global_metrics: HashMap<String, GlobalMetricConfig>,
    /// Per-metric leader assignment
    #[serde(default)]
    pub leaders: HashMap<u64, LeaderGroupConfig>,
    /// Bootstrap and discovery configuration
    pub bootstrap: BootstrapConfig,
    /// NEW: Node mapping customization (optional)
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

/// NEW: Global metric configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalMetricConfig {
    pub aggregation: String,           // sum, average, max, min
    #[serde(default = "default_true")]
    pub auto_aggregate: bool,          // Enable automatic calculation
    pub required_nodes: Option<usize>, // Minimum nodes for reliable aggregate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,    // Custom expression (uses existing engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_global_vars: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_local_vars: Option<Vec<String>>,
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
#[derive(Debug, Deserialize, Serialize)]
pub struct BootstrapConfig {
    pub anchor_nodes: Vec<u64>,         // Logical node IDs for anchor points
    pub discovery_port: u16,
    pub autoqueues_port: u16,           // Standard AutoQueues coordination port
    #[serde(default = "default_true")]
    pub enable_multicast: bool,         // UDP multicast discovery
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
        
        // Validate global metrics
        for (metric_name, metric_config) in &aq_config.global_metrics {
            match metric_config.aggregation.as_str() {
                "sum" | "average" | "max" | "min" => {},
                _ => return Err(ConfigError::ValidationError(format!(
                    "Invalid aggregation type '{}' for metric {}", 
                    metric_config.aggregation, metric_name
                ))),
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