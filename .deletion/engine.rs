//! Ultra-Minimal Generic Engine Trait
//!
//! Provides maximum flexibility with zero abstraction overhead.
//! Engines can process any input/output types while maintaining
//! simple execute() interface for evolutionary architecture.

use std::error::Error;

/// Ultra-minimal generic engine trait - maximum flexibility
pub trait Engine<Input, Output>: Send + Sync {
    /// Execute engine with given input, return output or error
    fn execute(&self, input: Input) -> Result<Output, Box<dyn Error + Send + Sync>>;

    /// Engine identifier for debugging/observability
    fn name(&self) -> &str;
}

/// Convenience type alias for engine trait
pub type GenericEngine<Input, Output> = Box<dyn Engine<Input, Output>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Test engine for mathematical expressions
    struct TestMathEngine;

    #[derive(Debug, Clone)]
    struct ExpressionInput {
        pub expression: String,
        pub variables: HashMap<String, f32>,
    }

    impl Engine<ExpressionInput, f32> for TestMathEngine {
        fn execute(&self, input: ExpressionInput) -> Result<f32, Box<dyn Error + Send + Sync>> {
            // Use expression for future parsing logic, for now sum variables
            let _expr = &input.expression;
            let sum: f32 = input.variables.values().sum();
            Ok(sum)
        }

        fn name(&self) -> &str {
            "TestMathEngine"
        }
    }

    #[test]
    fn test_engine_trait_implementation() {
        let engine = TestMathEngine;
        let input = ExpressionInput {
            expression: "(a + b) / 2".to_string(),
            variables: HashMap::from([("a".to_string(), 10.0), ("b".to_string(), 20.0)]),
        };

        let result = engine.execute(input).unwrap();
        assert_eq!(result, 30.0); // 10.0 + 20.0
        assert_eq!(engine.name(), "TestMathEngine");
    }

    #[test]
    fn test_engine_error_handling() {
        struct FailingEngine;

        #[derive(Debug)]
        struct SimpleInput;

        impl Engine<SimpleInput, f32> for FailingEngine {
            fn execute(&self, _input: SimpleInput) -> Result<f32, Box<dyn Error + Send + Sync>> {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Engine failed due to test condition",
                )) as Box<dyn Error + Send + Sync>)
            }

            fn name(&self) -> &str {
                "FailingEngine"
            }
        }

        let engine = FailingEngine;
        let result = engine.execute(SimpleInput);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "Engine failed due to test condition");
    }

    #[test]
    fn test_different_io_types() {
        // String processing engine
        struct StringEngine;

        impl Engine<String, String> for StringEngine {
            fn execute(&self, input: String) -> Result<String, Box<dyn Error + Send + Sync>> {
                Ok(input.to_uppercase())
            }

            fn name(&self) -> &str {
                "StringEngine"
            }
        }

        let engine = StringEngine;
        let result = engine.execute("hello world".to_string()).unwrap();
        assert_eq!(result, "HELLO WORLD");

        // Binary data engine
        struct BinaryEngine;

        impl Engine<Vec<u8>, usize> for BinaryEngine {
            fn execute(&self, input: Vec<u8>) -> Result<usize, Box<dyn Error + Send + Sync>> {
                Ok(input.len())
            }

            fn name(&self) -> &str {
                "BinaryEngine"
            }
        }

        let engine = BinaryEngine;
        let result = engine.execute(vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(result, 5);
    }
}
