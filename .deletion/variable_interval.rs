//! Variable Interval Trait for Adaptive Queue Management
//!
//! Provides unified interface for interval adaptation strategies including
//! AIMD, statistical analysis, and machine learning-based approaches.

pub trait VariableInterval: Send + Sync {
    /// Get current processing interval
    fn current_interval(&self) -> u64;

    /// Adjust interval based on system conditions
    fn adjust_interval(&mut self) -> Result<(), IntervalError>;

    /// Get interval bounds
    fn interval_bounds(&self) -> (u64, u64); // (min, max)

    /// Reset to specific interval
    fn reset(&mut self, interval: u64) -> Result<(), IntervalError>;

    /// Get adaptation statistics for monitoring
    fn adaptation_stats(&self) -> AdaptationStats;
}

/// Standard interval adaptation errors
#[derive(Debug, Clone)]
pub enum IntervalError {
    IntervalBelowMin(u64, u64),
    IntervalAboveMax(u64, u64),
    InvalidParameters(String),
    AdaptationFailed(String),
    DataUnavailable,
}

impl std::fmt::Display for IntervalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntervalError::IntervalBelowMin(interval, min) => {
                write!(f, "Interval {} below minimum {}", interval, min)
            }
            IntervalError::IntervalAboveMax(interval, max) => {
                write!(f, "Interval {} above maximum {}", interval, max)
            }
            IntervalError::InvalidParameters(msg) => {
                write!(f, "Invalid parameters: {}", msg)
            }
            IntervalError::AdaptationFailed(msg) => {
                write!(f, "Adaptation failed: {}", msg)
            }
            IntervalError::DataUnavailable => {
                write!(f, "Data unavailable for adaptation")
            }
        }
    }
}

impl std::error::Error for IntervalError {}

/// Adaptation statistics for monitoring
#[derive(Debug, Clone)]
pub struct AdaptationStats {
    pub current_interval: u64,
    pub min_interval: u64,
    pub max_interval: u64,
    pub total_adjustments: u64,
    pub increase_count: u64,
    pub decrease_count: u64,
    pub average_magnitude: f64,
    pub last_adjustment_ms: Option<u64>,
}

impl AdaptationStats {
    pub fn new(current: u64, min: u64, max: u64) -> Self {
        Self {
            current_interval: current,
            min_interval: min,
            max_interval: max,
            total_adjustments: 0,
            increase_count: 0,
            decrease_count: 0,
            average_magnitude: 0.0,
            last_adjustment_ms: None,
        }
    }

    pub fn record_adjustment(&mut self, old_interval: u64, new_interval: u64) {
        self.total_adjustments += 1;
        let magnitude = ((new_interval as f64 - old_interval as f64) / old_interval as f64).abs();

        if new_interval > old_interval {
            self.increase_count += 1;
        } else if new_interval < old_interval {
            self.decrease_count += 1;
        }

        // Update magnitude using incremental average
        self.average_magnitude = (self.average_magnitude * (self.total_adjustments - 1) as f64
            + magnitude)
            / self.total_adjustments as f64;
        self.current_interval = new_interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestVariableInterval {
        current: u64,
        min_interval: u64,
        max_interval: u64,
        total_adjustments: u64,
    }

    impl TestVariableInterval {
        fn new(initial: u64, min: u64, max: u64) -> Self {
            Self {
                current: initial,
                min_interval: min,
                max_interval: max,
                total_adjustments: 0,
            }
        }
    }

    impl VariableInterval for TestVariableInterval {
        fn current_interval(&self) -> u64 {
            self.current
        }

        fn adjust_interval(&mut self) -> Result<(), IntervalError> {
            self.total_adjustments += 1;
            // Simple logic: increment by 10%, bounded by min/max
            let new_value = (self.current as f64 * 1.1) as u64;
            self.current = new_value.max(self.min_interval).min(self.max_interval);
            Ok(())
        }

        fn interval_bounds(&self) -> (u64, u64) {
            (self.min_interval, self.max_interval)
        }

        fn reset(&mut self, interval: u64) -> Result<(), IntervalError> {
            if interval < self.min_interval {
                return Err(IntervalError::IntervalBelowMin(interval, self.min_interval));
            }
            if interval > self.max_interval {
                return Err(IntervalError::IntervalAboveMax(interval, self.max_interval));
            }
            self.current = interval;
            Ok(())
        }

        fn adaptation_stats(&self) -> AdaptationStats {
            let mut stats =
                AdaptationStats::new(self.current, self.min_interval, self.max_interval);
            stats.total_adjustments = self.total_adjustments;
            stats
        }
    }

    #[test]
    fn test_variable_interval_trait() {
        let mut controller = TestVariableInterval::new(1000, 100, 10000);

        assert_eq!(controller.current_interval(), 1000);
        assert_eq!(controller.interval_bounds(), (100, 10000));

        // First adjustment
        controller.adjust_interval().unwrap();
        assert_eq!(controller.current_interval(), 1100); // +10%

        // Test bounds
        controller.reset(50).unwrap_err(); // Below min
        controller.reset(50000).unwrap_err(); // Above max
        controller.reset(500).unwrap();
        assert_eq!(controller.current_interval(), 500);

        let stats = controller.adaptation_stats();
        assert_eq!(stats.total_adjustments, 1);
        assert_eq!(stats.current_interval, 500);
        assert_eq!(stats.min_interval, 100);
        assert_eq!(stats.max_interval, 10000);
    }

    #[test]
    fn test_adaptation_stats() {
        let mut stats = AdaptationStats::new(1000, 100, 10000);

        stats.record_adjustment(1000, 1100); // 10% increase
        assert_eq!(stats.total_adjustments, 1);
        assert_eq!(stats.increase_count, 1);
        assert_eq!(stats.decrease_count, 0);
        assert_eq!(stats.current_interval, 1100);
        assert_eq!(stats.average_magnitude, 0.1);

        stats.record_adjustment(1100, 880); // 20% decrease
        assert_eq!(stats.total_adjustments, 2);
        assert_eq!(stats.increase_count, 1);
        assert_eq!(stats.decrease_count, 1);
        assert!(stats.average_magnitude - 0.15 < f64::EPSILON);
    }
}
