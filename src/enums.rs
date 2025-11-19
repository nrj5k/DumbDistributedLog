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

// ============================================================================
// COMPREHENSIVE ENUM UNIT TESTS
// ============================================================================

#[cfg(test)]
mod enum_tests {
    use super::*;

    // Test Module 1: Mode Enum Operations
    #[test]
    fn test_mode_creation_and_validation() {
        // Test Mode enum creation
        let modes = vec![Mode::Sensor, Mode::Insight, Mode::Server, Mode::Client];

        for mode in modes {
            // All modes should be Debuggable
            let debug_str = format!("{:?}", mode);
            assert!(!debug_str.is_empty());

            // All modes should be Clone, Copy, PartialEq, Eq, Hash
            let cloned = mode.clone();
            assert_eq!(mode, cloned);
            let copied = mode;
            assert_eq!(mode, copied);
        }
    }

    #[test]
    fn test_mode_equality_operations() {
        // Test equality
        assert_eq!(Mode::Sensor, Mode::Sensor);
        assert_eq!(Mode::Insight, Mode::Insight);
        assert_eq!(Mode::Server, Mode::Server);
        assert_eq!(Mode::Client, Mode::Client);

        // Test inequality
        assert_ne!(Mode::Sensor, Mode::Insight);
        assert_ne!(Mode::Insight, Mode::Server);
        assert_ne!(Mode::Server, Mode::Client);
        assert_ne!(Mode::Client, Mode::Sensor);
    }

    #[test]
    fn test_mode_hash_consistency() {
        use std::collections::HashMap;

        // Test that Mode values can be used as HashMap keys
        let mut map = HashMap::new();

        let modes = vec![Mode::Sensor, Mode::Insight, Mode::Server, Mode::Client];

        for mode in modes {
            map.insert(mode, format!("test_{:?}", mode));
        }

        // Verify all modes can be retrieved
        assert_eq!(map.get(&Mode::Sensor), Some(&"test_Sensor".to_string()));
        assert_eq!(map.get(&Mode::Insight), Some(&"test_Insight".to_string()));
        assert_eq!(map.get(&Mode::Server), Some(&"test_Server".to_string()));
        assert_eq!(map.get(&Mode::Client), Some(&"test_Client".to_string()));

        // Test that equal modes have same hash
        let hash1 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            Mode::Sensor.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            Mode::Sensor.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2);
    }

    // Test Module 2: Model Enum Operations
    #[test]
    fn test_model_creation_and_validation() {
        // Test Model enum creation
        let models = vec![Model::Linear, Model::Quad, Model::Other];

        for model in models {
            // All models should be Debuggable
            let debug_str = format!("{:?}", model);
            assert!(!debug_str.is_empty());

            // All models should be Clone, Copy, PartialEq, Eq, Hash
            let cloned = model.clone();
            assert_eq!(model, cloned);
            let copied = model;
            assert_eq!(model, copied);
        }
    }

    #[test]
    fn test_model_equality_operations() {
        // Test equality
        assert_eq!(Model::Linear, Model::Linear);
        assert_eq!(Model::Quad, Model::Quad);
        assert_eq!(Model::Other, Model::Other);

        // Test inequality
        assert_ne!(Model::Linear, Model::Quad);
        assert_ne!(Model::Quad, Model::Other);
        assert_ne!(Model::Other, Model::Linear);
    }

    #[test]
    fn test_model_hash_consistency() {
        use std::collections::HashMap;

        // Test that Model values can be used as HashMap keys
        let mut map = HashMap::new();

        let models = vec![Model::Linear, Model::Quad, Model::Other];

        for model in models {
            map.insert(model, format!("test_{:?}", model));
        }

        // Verify all models can be retrieved
        assert_eq!(map.get(&Model::Linear), Some(&"test_Linear".to_string()));
        assert_eq!(map.get(&Model::Quad), Some(&"test_Quad".to_string()));
        assert_eq!(map.get(&Model::Other), Some(&"test_Other".to_string()));

        // Test that equal models have same hash
        let hash1 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            Model::Linear.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            Model::Linear.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2);
    }

    // Test Module 3: QueueValue Enum Operations
    #[test]
    fn test_queuevalue_creation_and_validation() {
        // Test QueueValue enum creation
        let values = vec![
            QueueValue::Undefined,
            QueueValue::NodeAvailability,
            QueueValue::NodeLoad,
            QueueValue::NodeCapacity,
            QueueValue::TierAvailability,
            QueueValue::TierLoad,
            QueueValue::TierCapacity,
            QueueValue::ClusterAvailability,
            QueueValue::ClusterLoad,
            QueueValue::ClusterCapacity,
            QueueValue::Sim,
        ];

        for value in values {
            // All values should be Debuggable
            let debug_str = format!("{:?}", value);
            assert!(!debug_str.is_empty());

            // All values should be Clone, Copy, PartialEq, Eq, Hash
            let cloned = value.clone();
            assert_eq!(value, cloned);
            let copied = value;
            assert_eq!(value, copied);
        }
    }

    #[test]
    fn test_queuevalue_equality_operations() {
        // Test equality
        assert_eq!(QueueValue::Undefined, QueueValue::Undefined);
        assert_eq!(QueueValue::NodeAvailability, QueueValue::NodeAvailability);
        assert_eq!(QueueValue::Sim, QueueValue::Sim);

        // Test inequality
        assert_ne!(QueueValue::NodeAvailability, QueueValue::NodeLoad);
        assert_ne!(QueueValue::ClusterLoad, QueueValue::ClusterCapacity);
        assert_ne!(QueueValue::Undefined, QueueValue::Sim);
    }

    #[test]
    fn test_queuevalue_hash_consistency() {
        use std::collections::HashMap;

        // Test that QueueValue values can be used as HashMap keys
        let mut map = HashMap::new();

        let values = vec![
            QueueValue::Undefined,
            QueueValue::NodeAvailability,
            QueueValue::NodeLoad,
            QueueValue::Sim,
        ];

        for value in values {
            map.insert(value, format!("test_{:?}", value));
        }

        // Verify all values can be retrieved
        assert_eq!(
            map.get(&QueueValue::Undefined),
            Some(&"test_Undefined".to_string())
        );
        assert_eq!(
            map.get(&QueueValue::NodeAvailability),
            Some(&"test_NodeAvailability".to_string())
        );
        assert_eq!(
            map.get(&QueueValue::NodeLoad),
            Some(&"test_NodeLoad".to_string())
        );
        assert_eq!(map.get(&QueueValue::Sim), Some(&"test_Sim".to_string()));

        // Test that equal values have same hash
        let hash1 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            QueueValue::NodeAvailability.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            QueueValue::NodeAvailability.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2);
    }

    // Test Module 4: ReverseTrieQueueNodeType Enum Operations
    #[test]
    fn test_rtq_node_type_creation_and_validation() {
        // Test ReverseTrieQueueNodeType enum creation
        let node_types = vec![
            ReverseTrieQueueNodeType::Undefined,
            ReverseTrieQueueNodeType::Root,
            ReverseTrieQueueNodeType::Node,
            ReverseTrieQueueNodeType::Leaf,
        ];

        for node_type in node_types {
            // All node types should be Debuggable
            let debug_str = format!("{:?}", node_type);
            assert!(!debug_str.is_empty());

            // All node types should be Clone, Copy, PartialEq, Eq, Hash
            let cloned = node_type.clone();
            assert_eq!(node_type, cloned);
            let copied = node_type;
            assert_eq!(node_type, copied);
        }
    }

    #[test]
    fn test_rtq_node_type_equality_operations() {
        // Test equality
        assert_eq!(
            ReverseTrieQueueNodeType::Undefined,
            ReverseTrieQueueNodeType::Undefined
        );
        assert_eq!(
            ReverseTrieQueueNodeType::Root,
            ReverseTrieQueueNodeType::Root
        );
        assert_eq!(
            ReverseTrieQueueNodeType::Node,
            ReverseTrieQueueNodeType::Node
        );
        assert_eq!(
            ReverseTrieQueueNodeType::Leaf,
            ReverseTrieQueueNodeType::Leaf
        );

        // Test inequality
        assert_ne!(
            ReverseTrieQueueNodeType::Undefined,
            ReverseTrieQueueNodeType::Root
        );
        assert_ne!(
            ReverseTrieQueueNodeType::Root,
            ReverseTrieQueueNodeType::Node
        );
        assert_ne!(
            ReverseTrieQueueNodeType::Node,
            ReverseTrieQueueNodeType::Leaf
        );
        assert_ne!(
            ReverseTrieQueueNodeType::Leaf,
            ReverseTrieQueueNodeType::Undefined
        );
    }

    #[test]
    fn test_rtq_node_type_hash_consistency() {
        use std::collections::HashMap;

        // Test that ReverseTrieQueueNodeType values can be used as HashMap keys
        let mut map = HashMap::new();

        let node_types = vec![
            ReverseTrieQueueNodeType::Undefined,
            ReverseTrieQueueNodeType::Root,
            ReverseTrieQueueNodeType::Node,
            ReverseTrieQueueNodeType::Leaf,
        ];

        for node_type in node_types {
            map.insert(node_type, format!("test_{:?}", node_type));
        }

        // Verify all node types can be retrieved
        assert_eq!(
            map.get(&ReverseTrieQueueNodeType::Undefined),
            Some(&"test_Undefined".to_string())
        );
        assert_eq!(
            map.get(&ReverseTrieQueueNodeType::Root),
            Some(&"test_Root".to_string())
        );
        assert_eq!(
            map.get(&ReverseTrieQueueNodeType::Node),
            Some(&"test_Node".to_string())
        );
        assert_eq!(
            map.get(&ReverseTrieQueueNodeType::Leaf),
            Some(&"test_Leaf".to_string())
        );

        // Test that equal node types have same hash
        let hash1 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            ReverseTrieQueueNodeType::Root.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            ReverseTrieQueueNodeType::Root.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2);
    }

    // Test Module 5: PredicateEnum Operations
    #[test]
    fn test_predicate_enum_creation_and_validation() {
        // Test PredicateEnum enum creation
        let predicates = vec![PredicateEnum::Last, PredicateEnum::TopNum];

        for predicate in predicates {
            // All predicates should be Debuggable
            let debug_str = format!("{:?}", predicate);
            assert!(!debug_str.is_empty());

            // All predicates should be Clone, Copy, PartialEq, Eq, Hash
            let cloned = predicate.clone();
            assert_eq!(predicate, cloned);
            let copied = predicate;
            assert_eq!(predicate, copied);
        }
    }

    #[test]
    fn test_predicate_enum_equality_operations() {
        // Test equality
        assert_eq!(PredicateEnum::Last, PredicateEnum::Last);
        assert_eq!(PredicateEnum::TopNum, PredicateEnum::TopNum);

        // Test inequality
        assert_ne!(PredicateEnum::Last, PredicateEnum::TopNum);
    }

    #[test]
    fn test_predicate_enum_hash_consistency() {
        use std::collections::HashMap;

        // Test that PredicateEnum values can be used as HashMap keys
        let mut map = HashMap::new();

        let predicates = vec![PredicateEnum::Last, PredicateEnum::TopNum];

        for predicate in predicates {
            map.insert(predicate, format!("test_{:?}", predicate));
        }

        // Verify all predicates can be retrieved
        assert_eq!(
            map.get(&PredicateEnum::Last),
            Some(&"test_Last".to_string())
        );
        assert_eq!(
            map.get(&PredicateEnum::TopNum),
            Some(&"test_TopNum".to_string())
        );

        // Test that equal predicates have same hash
        let hash1 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            PredicateEnum::Last.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            PredicateEnum::Last.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2);
    }

    // Test Module 6: Cross-Enum Compatibility Tests
    #[test]
    fn test_mode_model_compatibility() {
        // Test that Mode and Model can be used together in collections
        let modes = vec![Mode::Sensor, Mode::Insight, Mode::Server, Mode::Client];
        let models = vec![Model::Linear, Model::Quad, Model::Other];

        // All should be debuggable
        for mode in &modes {
            let debug_str = format!("{:?}", mode);
            assert!(!debug_str.is_empty());
        }

        for model in &models {
            let debug_str = format!("{:?}", model);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_queuevalue_with_mode_model() {
        // Test QueueValue with Mode and Model combinations
        let queue_values = vec![
            QueueValue::NodeAvailability,
            QueueValue::NodeLoad,
            QueueValue::NodeCapacity,
        ];

        let modes = vec![Mode::Sensor, Mode::Server];
        let models = vec![Model::Linear, Model::Quad];

        // All combinations should work without conflicts
        for qv in &queue_values {
            for mode in &modes {
                for model in &models {
                    // Just ensure they can be used together
                    let combined_str = format!("{:?}-{:?}-{:?}", qv, mode, model);
                    assert!(!combined_str.is_empty());
                }
            }
        }
    }

    // Test Module 7: Error Display Tests
    #[test]
    fn test_rtq_error_display() {
        // Test RTQError Display implementation
        let add_error = RTQError::AddQueueError("Test message".to_string());
        let other_error = RTQError::Other("Other message".to_string());

        let add_display = format!("{}", add_error);
        let other_display = format!("{}", other_error);

        assert_eq!(add_display, "Could not add Queue to RTQ: Test message");
        assert_eq!(other_display, "Other error: Other message");

        // Test Debug
        assert_eq!(
            format!("{:?}", add_error),
            "AddQueueError(\"Test message\")"
        );
        assert_eq!(format!("{:?}", other_error), "Other(\"Other message\")");
    }

    #[test]
    fn test_queue_error_display() {
        // Test QueueError Display implementation
        let unsupported_error = QueueError::UnsupportedOperation("Test operation");
        let not_found_error = QueueError::DataNotFound;
        let other_error = QueueError::Other("Other message".to_string());

        let unsupported_display = format!("{}", unsupported_error);
        let not_found_display = format!("{}", not_found_error);
        let other_display = format!("{}", other_error);

        assert_eq!(unsupported_display, "Unsupported operation: Test operation");
        assert_eq!(not_found_display, "Data not found");
        assert_eq!(other_display, "Other error: Other message");

        // Test Debug
        assert_eq!(
            format!("{:?}", unsupported_error),
            "UnsupportedOperation(\"Test operation\")"
        );
        assert_eq!(format!("{:?}", not_found_error), "DataNotFound");
        assert_eq!(format!("{:?}", other_error), "Other(\"Other message\")");
    }

    // Test Module 8: All Enum Variants Debug Tests
    #[test]
    fn test_enum_all_variants_debug() {
        // Test all Mode variants debug output
        assert_eq!(format!("{:?}", Mode::Sensor), "Sensor");
        assert_eq!(format!("{:?}", Mode::Insight), "Insight");
        assert_eq!(format!("{:?}", Mode::Server), "Server");
        assert_eq!(format!("{:?}", Mode::Client), "Client");

        // Test all Model variants debug output
        assert_eq!(format!("{:?}", Model::Linear), "Linear");
        assert_eq!(format!("{:?}", Model::Quad), "Quad");
        assert_eq!(format!("{:?}", Model::Other), "Other");
    }

    #[test]
    fn test_queue_value_all_variants_debug() {
        // Test all QueueValue variants debug output
        assert_eq!(format!("{:?}", QueueValue::Undefined), "Undefined");
        assert_eq!(
            format!("{:?}", QueueValue::NodeAvailability),
            "NodeAvailability"
        );
        assert_eq!(format!("{:?}", QueueValue::NodeLoad), "NodeLoad");
        assert_eq!(format!("{:?}", QueueValue::NodeCapacity), "NodeCapacity");
        assert_eq!(
            format!("{:?}", QueueValue::TierAvailability),
            "TierAvailability"
        );
        assert_eq!(format!("{:?}", QueueValue::TierLoad), "TierLoad");
        assert_eq!(format!("{:?}", QueueValue::TierCapacity), "TierCapacity");
        assert_eq!(
            format!("{:?}", QueueValue::ClusterAvailability),
            "ClusterAvailability"
        );
        assert_eq!(format!("{:?}", QueueValue::ClusterLoad), "ClusterLoad");
        assert_eq!(
            format!("{:?}", QueueValue::ClusterCapacity),
            "ClusterCapacity"
        );
        assert_eq!(format!("{:?}", QueueValue::Sim), "Sim");
    }

    #[test]
    fn test_rtq_node_type_all_variants_debug() {
        // Test all ReverseTrieQueueNodeType variants debug output
        assert_eq!(
            format!("{:?}", ReverseTrieQueueNodeType::Undefined),
            "Undefined"
        );
        assert_eq!(format!("{:?}", ReverseTrieQueueNodeType::Root), "Root");
        assert_eq!(format!("{:?}", ReverseTrieQueueNodeType::Node), "Node");
        assert_eq!(format!("{:?}", ReverseTrieQueueNodeType::Leaf), "Leaf");
    }

    #[test]
    fn test_predicate_enum_all_variants_debug() {
        // Test all PredicateEnum variants debug output
        assert_eq!(format!("{:?}", PredicateEnum::Last), "Last");
        assert_eq!(format!("{:?}", PredicateEnum::TopNum), "TopNum");
    }

    // Test Module 9: Performance and Memory Efficiency Tests
    #[test]
    fn test_enum_performance() {
        use std::time::Instant;

        // Test enum creation performance
        let start = Instant::now();
        for _ in 0..1000000 {
            let _mode = Mode::Sensor;
            let _model = Model::Linear;
            let _qv = QueueValue::NodeAvailability;
        }
        let duration = start.elapsed();
        // Should be reasonably fast (< 100ms for 1M operations)
        assert!(
            duration.as_millis() < 100,
            "Enum creation should be fast, got {}ms",
            duration.as_millis()
        );

        // Test enum comparison performance
        let start = Instant::now();
        for _ in 0..1000000 {
            let _result = Mode::Sensor == Mode::Sensor;
            let _result2 = Model::Linear != Model::Quad;
            let _result3 = QueueValue::NodeAvailability == QueueValue::NodeAvailability;
        }
        let duration = start.elapsed();
        // Should be reasonably fast (< 100ms for 1M comparisons)
        assert!(
            duration.as_millis() < 100,
            "Enum comparison should be fast, got {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_enum_memory_efficiency() {
        use std::mem;

        // Test memory size of enums
        assert_eq!(mem::size_of::<Mode>(), 1, "Mode should be 1 byte");
        assert_eq!(mem::size_of::<Model>(), 1, "Model should be 1 byte");
        assert_eq!(
            mem::size_of::<QueueValue>(),
            1,
            "QueueValue should be 1 byte (with 11 variants)"
        );
        assert_eq!(
            mem::size_of::<ReverseTrieQueueNodeType>(),
            1,
            "ReverseTrieQueueNodeType should be 1 byte"
        );
        assert_eq!(
            mem::size_of::<PredicateEnum>(),
            1,
            "PredicateEnum should be 1 byte"
        );

        // Test that enums can be copied efficiently
        let mode = Mode::Sensor;
        let copied_mode = mode;
        assert_eq!(mode, copied_mode);

        // Test that enums can be cloned efficiently
        let cloned_mode = mode.clone();
        assert_eq!(mode, cloned_mode);
    }

    // Test Module 10: Type Safety Tests
    #[test]
    fn test_enum_type_safety() {
        // Test that enums have proper type safety
        fn test_mode_type(_: Mode) {}
        fn test_model_type(_: Model) {}
        fn test_queuevalue_type(_: QueueValue) {}
        fn test_rtq_node_type(_: ReverseTrieQueueNodeType) {}
        fn test_predicate_type(_: PredicateEnum) {}

        // These should compile without errors
        test_mode_type(Mode::Sensor);
        test_model_type(Model::Linear);
        test_queuevalue_type(QueueValue::NodeAvailability);
        test_rtq_node_type(ReverseTrieQueueNodeType::Root);
        test_predicate_type(PredicateEnum::Last);
    }

    #[test]
    fn test_enum_copy_behavior() {
        // Test that enums with Copy trait behave correctly
        let mode = Mode::Sensor;
        let mode_copy = mode; // This should work with Copy trait
        assert_eq!(mode, mode_copy);
        assert_eq!(mode, Mode::Sensor); // Original unchanged

        let model = Model::Linear;
        let model_copy = model;
        assert_eq!(model, model_copy);
        assert_eq!(model, Model::Linear);

        let qv = QueueValue::NodeAvailability;
        let qv_copy = qv;
        assert_eq!(qv, qv_copy);
        assert_eq!(qv, QueueValue::NodeAvailability);
    }

    // Test Module 11: Mathematical Properties Tests
    #[test]
    fn test_equality_reflexivity() {
        // Test reflexivity: a == a
        assert_eq!(Mode::Sensor, Mode::Sensor);
        assert_eq!(Model::Linear, Model::Linear);
        assert_eq!(QueueValue::NodeAvailability, QueueValue::NodeAvailability);
        assert_eq!(
            ReverseTrieQueueNodeType::Root,
            ReverseTrieQueueNodeType::Root
        );
        assert_eq!(PredicateEnum::Last, PredicateEnum::Last);
    }

    #[test]
    fn test_equality_transitivity() {
        // Test transitivity: if a == b and b == c, then a == c
        let a = Mode::Sensor;
        let b = Mode::Sensor;
        let c = Mode::Sensor;
        let transitive_result = a == b && b == c;
        assert_eq!(transitive_result, a == c);

        let a = Model::Linear;
        let b = Model::Linear;
        let c = Model::Linear;
        assert_eq!(a == b && b == c, a == c);
    }

    #[test]
    fn test_equality_symmetry() {
        // Test symmetry: if a == b, then b == a
        assert_eq!(Mode::Sensor == Mode::Insight, Mode::Insight == Mode::Sensor);
        assert_eq!(Mode::Server == Mode::Client, Mode::Client == Mode::Server);

        assert_eq!(Model::Linear == Model::Quad, Model::Quad == Model::Linear);
        assert_eq!(Model::Quad == Model::Other, Model::Other == Model::Quad);

        assert_eq!(
            QueueValue::NodeAvailability == QueueValue::NodeLoad,
            QueueValue::NodeLoad == QueueValue::NodeAvailability
        );
        assert_eq!(
            QueueValue::TierLoad == QueueValue::TierAvailability,
            QueueValue::TierAvailability == QueueValue::TierLoad
        );

        assert_eq!(
            ReverseTrieQueueNodeType::Root == ReverseTrieQueueNodeType::Leaf,
            ReverseTrieQueueNodeType::Leaf == ReverseTrieQueueNodeType::Root
        );
    }

    #[test]
    fn test_hash_consistency() {
        use std::hash::{Hash, Hasher};

        // Test that equal values always have the same hash
        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();

        Mode::Sensor.hash(&mut hasher1);
        Mode::Sensor.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());

        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();

        Model::Linear.hash(&mut hasher1);
        Model::Linear.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());

        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();

        QueueValue::NodeAvailability.hash(&mut hasher1);
        QueueValue::NodeAvailability.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_hash_distribution() {
        use std::collections::HashMap;

        // Test hash distribution for different enum values
        let mut hash_counts = HashMap::new();

        // Collect hashes for all Mode variants
        use std::hash::{Hash, Hasher};
        for mode in [Mode::Sensor, Mode::Insight, Mode::Server, Mode::Client] {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            mode.hash(&mut hasher);
            let hash = hasher.finish();
            *hash_counts.entry(hash).or_insert(0) += 1;
        }

        // Should have 4 distinct hashes
        assert_eq!(hash_counts.len(), 4);

        // Clear for Model variants
        hash_counts.clear();
        for model in [Model::Linear, Model::Quad, Model::Other] {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            model.hash(&mut hasher);
            let hash = hasher.finish();
            *hash_counts.entry(hash).or_insert(0) += 1;
        }

        // Should have 3 distinct hashes
        assert_eq!(hash_counts.len(), 3);
    }
}
