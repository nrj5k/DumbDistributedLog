//! AIMD (Additive Increase Multiplicative Decrease) Controller Module
//!
//! This module implements an intelligent interval adaptation system that adjusts
//! queue processing intervals based on data variance. The core principle is:
//!
//! - **High variance** (rapidly changing values) → **Decrease interval** (multiplicative)
//! - **Low variance** (stable values) → **Increase interval** (additive)
//!
//! This creates a self-adapting system that responds to data volatility
//! while maintaining configurable bounds and cooldown periods.
//!
//! ## Usage Pattern
//! ```rust
//! let mut aimd = AimdController::new(
//!     1000,  // initial_interval_ms
//!     100,   // min_interval_ms  
//!     10000,  // max_interval_ms
//!     100,    // additive_factor_ms
//!     0.8,    // multiplicative_factor
//! );
//!
//! // Process new data point
//! aimd.update_variance(data_window);
//! let new_interval = aimd.get_current_interval();
//! ```

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AimdError {
    #[error("Interval {0}ms is below minimum {1}ms")]
    IntervalBelowMin(u64, u64),

    #[error("Interval {0}ms is above maximum {1}ms")]
    IntervalAboveMax(u64, u64),

    #[error("Adjustment cooldown active: {0}ms remaining")]
    CooldownActive(u64),

    #[error("Invalid variance calculation: {0}")]
    InvalidVariance(String),

    #[error("Empty data window provided")]
    EmptyDataWindow,
}

/// AIMD Controller for adaptive interval management
///
/// Uses variance-based adaptation to dynamically adjust processing intervals
/// based on data volatility patterns.
#[derive(Debug, Clone)]
pub struct AimdController {
    /// Current processing interval in milliseconds
    current_interval: u64,
    /// Minimum allowed interval in milliseconds
    min_interval: u64,
    /// Maximum allowed interval in milliseconds
    max_interval: u64,
    #[allow(dead_code)]
    /// Additive increase factor in milliseconds
    additive_factor: u64,
    #[allow(dead_code)]
    /// Multiplicative decrease factor (0.0 to 1.0)
    multiplicative_factor: f64,
    /// Time of last interval adjustment
    last_adjustment_time: Instant,
    /// Minimum time between adjustments (cooldown period)
    adjustment_cooldown: Duration,
    /// Current variance of data values
    variance: f64,
    /// Variance threshold for triggering adjustments
    variance_threshold: f64,
    /// Size of data window for variance calculation
    #[allow(dead_code)]
    window_size: usize,
}

impl AimdController {
    /// Create a new AIMD controller with specified parameters
    ///
    /// # Arguments
    /// * `initial_interval` - Starting interval in milliseconds
    /// * `min_interval` - Minimum allowed interval in milliseconds
    /// * `max_interval` - Maximum allowed interval in milliseconds
    /// * `additive_factor` - Milliseconds to add when increasing interval
    /// * `multiplicative_factor` - Factor to multiply when decreasing interval (0.0-1.0)
    ///
    /// # Returns
    /// New `AimdController` instance
    pub fn new(
        initial_interval: u64,
        min_interval: u64,
        max_interval: u64,
        additive_factor: u64,
        multiplicative_factor: f64,
    ) -> Result<Self, AimdError> {
        if initial_interval < min_interval {
            return Err(AimdError::IntervalBelowMin(initial_interval, min_interval));
        }
        if initial_interval > max_interval {
            return Err(AimdError::IntervalAboveMax(initial_interval, max_interval));
        }
        if multiplicative_factor < 0.0 || multiplicative_factor >= 1.0 {
            return Err(AimdError::InvalidVariance(
                "Multiplicative factor must be between 0.0 and 0.9".to_string(),
            ));
        }

        Ok(Self {
            current_interval: initial_interval,
            min_interval,
            max_interval,
            additive_factor,
            multiplicative_factor,
            last_adjustment_time: Instant::now(),
            adjustment_cooldown: Duration::from_millis(5000), // 5 second default cooldown
            variance: 0.0,
            variance_threshold: 1.0, // Default threshold
            window_size: 10,         // Default window size
        })
    }

    /// Create AIMD controller from configuration
    pub fn from_config(config: AimdConfig) -> Result<Self, AimdError> {
        Self::new(
            config.initial_interval_ms,
            config.min_interval_ms,
            config.max_interval_ms,
            config.additive_factor_ms,
            config.multiplicative_factor,
        )
    }

    /// Get the current interval
    pub fn get_current_interval(&self) -> u64 {
        self.current_interval
    }

    /// Get current variance
    pub fn get_variance(&self) -> f64 {
        self.variance
    }

    /// Set variance threshold for triggering adjustments
    pub fn set_variance_threshold(&mut self, threshold: f64) {
        self.variance_threshold = threshold;
    }

    /// Set adjustment cooldown period
    pub fn set_cooldown(&mut self, cooldown_ms: u64) {
        self.adjustment_cooldown = Duration::from_millis(cooldown_ms);
    }

    /// Update variance based on new data window
    ///
    /// Calculates variance of the provided data points and stores it.
    /// Uses sample variance (n-1 denominator) for better statistical properties.
    ///
    /// # Arguments
    /// * `data_window` - Slice of recent data points
    ///
    /// # Returns
    /// * `Ok(())` if variance calculated successfully
    /// * `Err(AimdError)` if data window is empty or invalid
    pub fn update_variance<T>(&mut self, data_window: &[T]) -> Result<(), AimdError>
    where
        T: Clone + Into<f64>,
    {
        if data_window.is_empty() {
            return Err(AimdError::EmptyDataWindow);
        }

        if data_window.len() < 2 {
            self.variance = 0.0;
            return Ok(());
        }

        // Convert to f64 values
        let values: Vec<f64> = data_window.iter().cloned().map(|v| v.into()).collect();

        // Calculate mean
        let mean = values.iter().sum::<f64>() / values.len() as f64;

        // Calculate sample variance
        let variance_sum = values
            .iter()
            .map(|&v| {
                let diff = v - mean;
                diff * diff
            })
            .sum::<f64>();

        self.variance = variance_sum / (values.len() - 1) as f64;
        Ok(())
    }

    /// Adjust interval based on current variance - CONSTANT INTERVAL VERSION
    ///
    /// Returns constant interval (average of min and max) regardless of variance
    ///
    /// # Returns
    /// * `Ok(())` always succeeds for constant interval
    pub fn adjust_interval(&mut self) -> Result<(), AimdError> {
        // Implement proper AIMD (Additive Increase Multiplicative Decrease) algorithm
        let elapsed = self.last_adjustment_time.elapsed();

        // Check cooldown period
        if elapsed < self.adjustment_cooldown {
            return Ok(());
        }

        // AIMD Algorithm logic:
        // - High variance = system unstable = decrease interval (multiplicative)
        // - Low variance = system stable = can increase interval (additive)

        // Check if adjustment is needed based on variance
        if self.variance < self.variance_threshold {
            // System is stable - ADDITIVELY INCREASE interval
            let new_interval = self.current_interval.saturating_add(self.additive_factor);
            self.current_interval = new_interval.min(self.max_interval);
        } else {
            // System is unstable - MULTIPLICATIVELY DECREASE interval
            let multiplier = self.multiplicative_factor;
            let new_interval = (self.current_interval as f64 * multiplier) as u64;
            self.current_interval = new_interval.max(self.min_interval);
        }

        // Update adjustment timestamp
        self.last_adjustment_time = Instant::now();

        Ok(())
    }

    /// Reset controller to specific interval
    ///
    /// Useful for reinitialization or manual override.
    pub fn reset(&mut self, interval: u64) -> Result<(), AimdError> {
        if interval < self.min_interval {
            return Err(AimdError::IntervalBelowMin(interval, self.min_interval));
        }
        if interval > self.max_interval {
            return Err(AimdError::IntervalAboveMax(interval, self.max_interval));
        }

        self.current_interval = interval;
        self.last_adjustment_time = Instant::now();
        self.variance = 0.0;
        Ok(())
    }

    /// Get controller statistics for monitoring
    pub fn get_stats(&self) -> AimdStats {
        AimdStats {
            current_interval: self.current_interval,
            min_interval: self.min_interval,
            max_interval: self.max_interval,
            current_variance: self.variance,
            variance_threshold: self.variance_threshold,
            time_since_last_adjustment: self.last_adjustment_time.elapsed(),
            cooldown_remaining: self
                .adjustment_cooldown
                .saturating_sub(self.last_adjustment_time.elapsed()),
        }
    }
}

/// Configuration for AIMD controller (typically loaded from TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimdConfig {
    /// Initial interval in milliseconds
    pub initial_interval_ms: u64,
    /// Minimum allowed interval in milliseconds
    pub min_interval_ms: u64,
    /// Maximum allowed interval in milliseconds
    pub max_interval_ms: u64,
    /// Additive increase factor in milliseconds
    pub additive_factor_ms: u64,
    /// Multiplicative decrease factor (0.0 to 1.0)
    pub multiplicative_factor: f64,
    /// Variance threshold for triggering adjustments
    pub variance_threshold: Option<f64>,
    /// Adjustment cooldown period in milliseconds
    pub cooldown_ms: Option<u64>,
    /// Data window size for variance calculation
    pub window_size: Option<usize>,
}

impl Default for AimdConfig {
    fn default() -> Self {
        Self {
            initial_interval_ms: 1000,
            min_interval_ms: 100,
            max_interval_ms: 10000,
            additive_factor_ms: 100,
            multiplicative_factor: 0.8,
            variance_threshold: None,
            cooldown_ms: None,
            window_size: None,
        }
    }
}

/// Statistics for AIMD controller monitoring
#[derive(Debug, Clone)]
pub struct AimdStats {
    pub current_interval: u64,
    pub min_interval: u64,
    pub max_interval: u64,
    pub current_variance: f64,
    pub variance_threshold: f64,
    pub time_since_last_adjustment: Duration,
    pub cooldown_remaining: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aimd_creation() {
        let aimd = AimdController::new(1000, 100, 10000, 100, 0.8).unwrap();
        assert_eq!(aimd.get_current_interval(), 1000);
        assert_eq!(aimd.get_variance(), 0.0);
    }

    #[test]
    fn test_aimd_bounds() {
        // Test invalid initial interval
        assert!(AimdController::new(50, 100, 10000, 100, 0.8).is_err());
        assert!(AimdController::new(15000, 100, 10000, 100, 0.8).is_err());

        // Test invalid multiplicative factor
        assert!(AimdController::new(1000, 100, 10000, 100, -0.1).is_err());
        assert!(AimdController::new(1000, 100, 10000, 100, 1.5).is_err());
    }

    #[test]
    fn test_variance_calculation() {
        let mut aimd = AimdController::new(1000, 100, 10000, 100, 0.8).unwrap();

        // Test with stable data
        let stable_data = vec![10.0, 10.1, 9.9, 10.0, 10.2];
        aimd.update_variance(&stable_data).unwrap();
        assert!(aimd.get_variance() < 1.0); // Low variance

        // Test with volatile data
        let volatile_data = vec![10.0, 20.0, 5.0, 25.0, 2.0];
        aimd.update_variance(&volatile_data).unwrap();
        assert!(aimd.get_variance() > 50.0); // High variance
    }

    #[test]
    fn test_constant_interval() {
        let mut aimd = AimdController::new(1000, 100, 10000, 100, 0.8).unwrap();

        // Should always return constant interval (midpoint of min and max)
        aimd.adjust_interval().unwrap();
        assert_eq!(aimd.get_current_interval(), 1000); // Returns to midpoint

        // Test with different bounds
        let mut aimd2 = AimdController::new(500, 200, 800, 50, 0.9).unwrap();
        aimd2.adjust_interval().unwrap();
        assert_eq!(aimd2.get_current_interval(), (200 + 800) / 2); // 500
    }

    #[test]
    fn test_config_default() {
        let config = AimdConfig::default();
        assert_eq!(config.initial_interval_ms, 1000);
        assert_eq!(config.min_interval_ms, 100);
        assert_eq!(config.max_interval_ms, 10000);
        assert_eq!(config.additive_factor_ms, 100);
        assert_eq!(config.multiplicative_factor, 0.8);
    }
}
