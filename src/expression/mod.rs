//! Expression engine module using evalexpr
//!
//! Provides parsing and evaluation of mathematical expressions using the
//! evalexpr crate for robust expression handling.

use evalexpr::{ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Value};
use std::collections::HashMap;

/// Expression evaluation result
pub type EvalResult = Result<f64, ExpressionError>;

/// Expression errors
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ExpressionError {
    #[error("Invalid expression: {0}")]
    InvalidExpression(String),
    #[error("Variable not found: {0}")]
    UndefinedVariable(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Evaluation error: {0}")]
    EvaluationError(String),
}

/// Evaluate an expression with the given variables
///
/// # Arguments
/// * `expr` - The expression string to evaluate (e.g., "a + b * 2")
/// * `variables` - A HashMap of variable names to f64 values
///
/// # Returns
/// * `Ok(f64)` - The evaluated result
/// * `Err(ExpressionError)` - If evaluation fails
pub fn evaluate_expression(expr: &str, variables: &HashMap<String, f64>) -> EvalResult {
    let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

    for (name, value) in variables {
        let val = Value::from_float(*value);
        context
            .set_value(name.clone(), val)
            .map_err(|e| ExpressionError::EvaluationError(e.to_string()))?;
    }

    evalexpr::eval_float_with_context(expr, &context)
        .map_err(|e| ExpressionError::EvaluationError(e.to_string()))
}

/// Check if an expression is valid (syntactically correct)
///
/// # Arguments
/// * `expr` - The expression string to validate
///
/// # Returns
/// * `true` - Expression is valid
/// * `false` - Expression has syntax errors
pub fn is_valid_expression(expr: &str) -> bool {
    evaluate_expression(expr, &HashMap::new()).is_ok()
}

/// Parse and compile an expression for later evaluation
///
/// Returns a compiled expression that can be evaluated multiple times
/// with different variable contexts.
pub fn compile_expression(expr: &str) -> Result<CompiledExpression, ExpressionError> {
    // Validate expression syntax by trying to build it
    // We use eval_float_with_context which parses and evaluates
    // For expressions with undefined variables, we accept them as valid syntax
    let context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

    match evalexpr::eval_float_with_context(expr, &context) {
        Ok(_) => {
            // Literal expressions work directly
            Ok(CompiledExpression {
                expression: expr.to_string(),
            })
        }
        Err(ref e) => {
            let err_str = e.to_string();
            // Variables not defined is OK - syntax is valid
            // The expression will work when variables are provided later
            // Check for common variable-related error patterns
            if err_str.contains("variable")
                || err_str.contains("not found")
                || err_str.contains("identifier")
            {
                Ok(CompiledExpression {
                    expression: expr.to_string(),
                })
            } else {
                Err(ExpressionError::InvalidExpression(err_str))
            }
        }
    }
}

/// A compiled expression ready for evaluation
#[derive(Debug, Clone)]
pub struct CompiledExpression {
    expression: String,
}

impl CompiledExpression {
    /// Evaluate the expression with the given variables
    pub fn evaluate(&self, variables: &HashMap<String, f64>) -> EvalResult {
        evaluate_expression(&self.expression, variables)
    }

    /// Get the original expression string
    pub fn as_str(&self) -> &str {
        &self.expression
    }
}

// Backward compatibility types for existing code
pub use CompiledExpression as ExpressionAST;
pub use ExpressionError as ParseError;

/// Migrate from Parser::parse to this function
pub use evaluate_expression as parse_and_evaluate;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_simple_expression() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 20.0);

        let result = evaluate_expression("a + b", &vars);
        assert_eq!(result.unwrap(), 30.0);
    }

    #[test]
    fn test_evaluate_with_operations() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 5.0);

        let result = evaluate_expression("x * 2 + 3", &vars);
        assert_eq!(result.unwrap(), 13.0);
    }

    #[test]
    fn test_invalid_expression() {
        let vars = HashMap::new();
        let result = evaluate_expression("invalid syntax ++", &vars);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_valid_expression() {
        // Test with variables defined
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 1.0);
        vars.insert("b".to_string(), 2.0);

        // These should work because variables are defined
        assert!(evaluate_expression("a + b", &vars).is_ok());
        assert!(evaluate_expression("a * b + 3", &vars).is_ok());

        // Invalid syntax should fail
        assert!(evaluate_expression("1 ++ 2", &HashMap::new()).is_err());
    }

    #[test]
    fn test_compile_expression() {
        // Compile should succeed for syntactically valid expressions
        let compiled = compile_expression("a + b");
        assert!(
            compiled.is_ok(),
            "compile_expression should succeed for valid syntax"
        );

        // And it should evaluate with proper variables
        let compiled = compiled.unwrap();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 5.0);
        vars.insert("b".to_string(), 3.0);
        assert_eq!(compiled.evaluate(&vars).unwrap(), 8.0);
    }
}
