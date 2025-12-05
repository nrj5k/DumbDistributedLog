//! Unified Configuration System - KISS Simplified
//!
//! Single configuration file following KISS principle.
//! Ultra-minimal design with TOML support for queue expressions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

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
            ConfigError::ParseError(msg) => write!(f, "TOML parse error: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Config validation error: {}", msg),
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

/// Main configuration structure
#[derive(Debug, Deserialize, Serialize)]
pub struct QueueConfig {
    pub queues: QueuesSection,
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

impl QueueConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
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

    /// Create standard configuration
    pub fn create_standard() -> Self {
        let toml_str = r#"
[queues]
base = ["cpu_percent", "memory_percent", "drive_percent"]

[queues.derived.health_score]
formula = "(local.cpu_percent + local.memory_percent + local.drive_percent) / 3.0"

[queues.derived.weighted_health]
formula = "(local.cpu_percent * 0.4) + (local.memory_percent * 0.3) + (local.drive_percent * 0.3)"
"#;
        Self::from_str(toml_str).expect("Standard config should be valid")
    }

    /// Create minimal configuration
    pub fn create_minimal() -> Self {
        let toml_str = r#"
[queues]
base = ["cpu_percent", "memory_percent"]

[queues.derived.simple_average]
formula = "(local.cpu_percent + local.memory_percent) / 2.0"
"#;
        Self::from_str(toml_str).expect("Minimal config should be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_toml = r#"
[queues]
base = ["cpu_percent", "memory_percent"]

[queues.derived.simple]
formula = "(local.cpu_percent + local.memory_percent) / 2.0"
"#;

        let config = QueueConfig::from_str(config_toml).unwrap();
        assert_eq!(
            config.get_base_metrics(),
            &["cpu_percent", "memory_percent"]
        );
        assert_eq!(
            config.get_derived_formula("simple"),
            Some("(local.cpu_percent + local.memory_percent) / 2.0")
        );
    }

    #[test]
    fn test_standard_config() {
        let config = QueueConfig::create_standard();
        assert!(!config.get_base_metrics().is_empty());
        assert!(!config.get_derived_formulas().collect::<Vec<_>>().is_empty());
    }
}
