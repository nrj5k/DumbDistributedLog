//! Interval configuration for queue polling with AIMD (Adaptive) support
//!
//! Provides configurable polling intervals with two modes:
//! 1. Constant: Fixed interval polling
//! 2. Adaptive: AIMD-based adaptive interval that adjusts based on queue performance

use std::time::Duration;

/// Polling interval configuration
#[derive(Debug, Clone)]
pub enum IntervalConfig {
    /// Constant interval in milliseconds
    Constant(u64),

    /// Adaptive interval using AIMD (Additive Increase Multiplicative Decrease)
    Adaptive {
        /// Initial interval in milliseconds
        initial_ms: u64,
        /// Minimum interval (fastest polling)
        min_ms: u64,
        /// Maximum interval (slowest polling)
        max_ms: u64,
        /// Additive increase step (decrease interval by this much on success)
        additive_step_ms: u64,
        /// Multiplicative decrease factor (multiply interval by this on overflow)
        /// Should be > 1.0, typically 2.0
        multiplicative_factor: f64,
    },
}

impl Default for IntervalConfig {
    fn default() -> Self {
        Self::Constant(1000) // Default 1 second
    }
}

/// AIMD controller for adaptive polling
pub struct AimdController {
    config: IntervalConfig,
    current_interval_ms: u64,
    consecutive_successes: u32,
}

impl AimdController {
    pub fn new(config: IntervalConfig) -> Self {
        let initial_ms = match &config {
            IntervalConfig::Constant(ms) => *ms,
            IntervalConfig::Adaptive { initial_ms, .. } => *initial_ms,
        };

        Self {
            config,
            current_interval_ms: initial_ms,
            consecutive_successes: 0,
        }
    }

    /// Get current interval duration
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.current_interval_ms)
    }

    /// Called when poll succeeds (data pushed to queue without overflow)
    /// Additively decreases interval (polls faster)
    pub fn on_success(&mut self) {
        match &self.config {
            IntervalConfig::Constant(_) => {} // No change for constant
            IntervalConfig::Adaptive {
                min_ms,
                additive_step_ms,
                ..
            } => {
                self.consecutive_successes += 1;
                // Additively decrease interval (poll faster)
                if self.current_interval_ms > *min_ms {
                    self.current_interval_ms =
                        self.current_interval_ms.saturating_sub(*additive_step_ms);
                    self.current_interval_ms = self.current_interval_ms.max(*min_ms);
                }
            }
        }
    }

    /// Called when poll fails or queue overflows
    /// Multiplicatively increases interval (polls slower)
    pub fn on_overflow(&mut self) {
        match &self.config {
            IntervalConfig::Constant(_) => {} // No change for constant
            IntervalConfig::Adaptive {
                max_ms,
                multiplicative_factor,
                ..
            } => {
                self.consecutive_successes = 0;
                // Multiplicatively increase interval (poll slower)
                let new_interval = (self.current_interval_ms as f64 * multiplicative_factor) as u64;
                self.current_interval_ms = new_interval.min(*max_ms);
            }
        }
    }

    /// Get current interval for reporting/debugging
    pub fn current_interval_ms(&self) -> u64 {
        self.current_interval_ms
    }
}
