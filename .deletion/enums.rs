use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueValue {
    Undefined,
    NodeAvailability,
    NodeLoad,
    NodeCapacity,
    TierAvailability,
    TierLoad,
    TierCapacity,
    ClusterAvailability,
    ClusterLoad,
    ClusterCapacity,
    SystemHealth,
    Sim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReverseTrieQueueNodeType {
    Undefined,
    Root,
    Node,
    Leaf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredicateEnum {
    Last,
    TopNum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Sensor,
    Insight,
    Server,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub enum Model {
    Linear,
    Quad,
    Other,
}

#[derive(Debug)]
pub enum RTQError {
    AddQueueError(String),
    Other(String),
}

impl fmt::Display for RTQError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RTQError::AddQueueError(msg) => write!(f, "Could not add Queue to RTQ: {}", msg),
            RTQError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub enum QueueError {
    UnsupportedOperation(&'static str),
    DataNotFound,
    Other(String),
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
            QueueError::DataNotFound => write!(f, "Data not found"),
            QueueError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl Error for QueueError {}

// Essential unit tests only
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_creation_and_equality() {
        let modes = vec![Mode::Sensor, Mode::Insight, Mode::Server, Mode::Client];

        for mode in modes {
            let cloned = mode.clone();
            assert_eq!(mode, cloned);
        }

        assert_eq!(Mode::Sensor, Mode::Sensor);
        assert_ne!(Mode::Sensor, Mode::Insight);
    }

    #[test]
    fn test_model_creation_and_equality() {
        let models = vec![Model::Linear, Model::Quad, Model::Other];

        for model in models {
            let cloned = model.clone();
            assert_eq!(model, cloned);
        }

        assert_eq!(Model::Linear, Model::Linear);
        assert_ne!(Model::Linear, Model::Quad);
    }

    #[test]
    fn test_queue_value_creation_and_equality() {
        let values = vec![
            QueueValue::Undefined,
            QueueValue::NodeAvailability,
            QueueValue::NodeLoad,
            QueueValue::Sim,
        ];

        for value in values {
            let cloned = value.clone();
            assert_eq!(value, cloned);
        }

        assert_eq!(QueueValue::NodeAvailability, QueueValue::NodeAvailability);
        assert_ne!(QueueValue::NodeAvailability, QueueValue::NodeLoad);
    }

    #[test]
    fn test_rtq_error_display() {
        let add_error = RTQError::AddQueueError("Test message".to_string());
        let other_error = RTQError::Other("Other message".to_string());

        assert_eq!(
            format!("{}", add_error),
            "Could not add Queue to RTQ: Test message"
        );
        assert_eq!(format!("{}", other_error), "Other error: Other message");
    }

    #[test]
    fn test_queue_error_display() {
        let unsupported_error = QueueError::UnsupportedOperation("Test operation");
        let not_found_error = QueueError::DataNotFound;
        let other_error = QueueError::Other("Other message".to_string());

        assert_eq!(
            format!("{}", unsupported_error),
            "Unsupported operation: Test operation"
        );
        assert_eq!(format!("{}", not_found_error), "Data not found");
        assert_eq!(format!("{}", other_error), "Other error: Other message");
    }
}
