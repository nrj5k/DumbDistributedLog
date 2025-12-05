//! Simplified External Sensor Module
//!
//! Focuses on demonstrating Engine trait usage for external data processing.

use crate::engine::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

/// External sensor data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalSensorData {
    pub sensor_id: String,
    pub value: f64,
    pub timestamp: u64,
    pub metadata: Option<serde_json::Value>,
}

/// Sensor analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorAnalysis {
    pub sensor_id: String,
    pub average_value: f64,
    pub anomaly_score: f64,
    pub status: SensorStatus,
}

/// Sensor status classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorStatus {
    Normal,
    Warning,
    Critical,
    Unknown,
}

/// Engine for analyzing external sensor data
pub struct SensorAnalysisEngine {
    pub name: String,
}

impl Engine<Vec<ExternalSensorData>, Vec<SensorAnalysis>> for SensorAnalysisEngine {
    fn execute(
        &self,
        input: Vec<ExternalSensorData>,
    ) -> Result<Vec<SensorAnalysis>, Box<dyn Error + Send + Sync>> {
        let mut results = Vec::new();

        // Group by sensor_id and analyze each group
        let mut sensor_groups: HashMap<String, Vec<ExternalSensorData>> = HashMap::new();

        for data in input {
            sensor_groups
                .entry(data.sensor_id.clone())
                .or_insert_with(Vec::new)
                .push(data);
        }

        for (sensor_id, readings) in sensor_groups {
            if readings.is_empty() {
                continue;
            }

            // Calculate average
            let sum: f64 = readings.iter().map(|r| r.value).sum();
            let average = sum / readings.len() as f64;

            // Simple anomaly detection: high variance indicates potential issues
            let variance = calculate_variance(&readings, average);
            let anomaly_score = (variance / (average.max(1.0))).min(1.0);

            // Determine status based on value ranges
            let status = match average {
                v if v < 10.0 => SensorStatus::Normal,
                v if v < 50.0 => SensorStatus::Warning,
                v if v < 100.0 => SensorStatus::Critical,
                _ => SensorStatus::Unknown,
            };

            results.push(SensorAnalysis {
                sensor_id: sensor_id.clone(),
                average_value: average,
                anomaly_score: anomaly_score,
                status: status,
            });
        }

        Ok(results)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Engine for filtering sensor data by thresholds
pub struct ThresholdFilterEngine {
    pub name: String,
    pub threshold: f64,
    pub comparison: ThresholdComparison,
}

#[derive(Debug, Clone)]
pub enum ThresholdComparison {
    Above,
    Below,
    Equal,
}

impl Engine<Vec<ExternalSensorData>, Vec<ExternalSensorData>> for ThresholdFilterEngine {
    fn execute(
        &self,
        input: Vec<ExternalSensorData>,
    ) -> Result<Vec<ExternalSensorData>, Box<dyn Error + Send + Sync>> {
        let mut filtered = Vec::new();

        for data in input {
            let should_include = match self.comparison {
                ThresholdComparison::Above => data.value > self.threshold,
                ThresholdComparison::Below => data.value < self.threshold,
                ThresholdComparison::Equal => (data.value - self.threshold).abs() < 0.001,
            };

            if should_include {
                filtered.push(data);
            }
        }

        Ok(filtered)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn calculate_variance(readings: &[ExternalSensorData], mean: f64) -> f64 {
    if readings.len() <= 1 {
        return 0.0;
    }

    let sum_squared_diff: f64 = readings.iter().map(|r| (r.value - mean).powi(2)).sum();

    sum_squared_diff / (readings.len() as f64 - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_analysis_engine() {
        let engine = SensorAnalysisEngine {
            name: "TestSensor".to_string(),
        };

        let input = vec![
            ExternalSensorData {
                sensor_id: "sensor1".to_string(),
                value: 25.0,
                timestamp: 1000,
                metadata: None,
            },
            ExternalSensorData {
                sensor_id: "sensor1".to_string(),
                value: 25.0,
                timestamp: 2000,
                metadata: None,
            },
        ];

        let result = engine.execute(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sensor_id, "sensor1");
        assert_eq!(result[0].average_value, 25.0);
    }
}
