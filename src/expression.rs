//! Expression Engine - KISS Simplified
//!
//! Ultra-minimal expression system with 3-method trait.
//! Supports basic arithmetic with local/global variables.

use eval::Expr;
use std::collections::HashMap;

/// Expression errors
#[derive(Debug, Clone)]
pub enum ExpressionError {
    ParseError(String),
    UndefinedVariable(String),
    DivisionByZero(String),
}

impl std::fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpressionError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ExpressionError::UndefinedVariable(var) => write!(f, "Undefined variable: {}", var),
            ExpressionError::DivisionByZero(msg) => write!(f, "Division by zero in: {}", msg),
        }
    }
}

impl std::error::Error for ExpressionError {}

/// Ultra-minimal expression trait for maximum flexibility
pub trait Expression<T>: Send + Sync {
    fn convert_syntax(&self, expr: &str) -> Result<String, ExpressionError> {
        let mut converted = expr.to_string();

        // Replace local.variable with local_variable (eval doesn't like dots)
        converted = converted.replace("local.", "local_");
        converted = converted.replace("global.", "global_");

        Ok(converted)
    }
    /// Evaluate expression with given variable contexts
    fn evaluate(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Result<T, ExpressionError>;

    /// Get required local variables
    fn required_local_vars(&self) -> Vec<String>;

    /// Get required global variables
    fn required_global_vars(&self) -> Vec<String>;
}

/// Type alias for f64 expressions (most common use case)
pub type ExpressionF64 = SimpleExpression<f64>;

/// Simple expression implementation
#[derive(Debug, Clone)]
pub struct SimpleExpression<T> {
    pub expression: String,
    _phantom: std::marker::PhantomData<T>,
}

impl SimpleExpression<f64> {
    /// Create new f64 expression (convenience method)
    pub fn new_f64(expression: &str) -> Result<Self, ExpressionError> {
        Self::new(expression)
    }
}

impl<T: Clone + Send + 'static> SimpleExpression<T> {
    /// Create new expression with comprehensive validation
    pub fn new(expression: &str) -> Result<Self, ExpressionError> {
        let expr = expression.trim();

        if expr.is_empty() {
            return Err(ExpressionError::ParseError("Empty expression".to_string()));
        }

        // Create the expression object first
        let simple_expr = Self {
            expression: expr.to_string(),
            _phantom: std::marker::PhantomData,
        };

        // Run comprehensive validation
        simple_expr.validate_expression()?;

        Ok(simple_expr)
    }

    /// Comprehensive expression validation with detailed error reporting
    fn validate_expression(&self) -> Result<(), ExpressionError> {
        let expr = self.expression.trim();

        // Basic validation - empty expression
        if expr.is_empty() {
            return Err(ExpressionError::ParseError(
                "Expression cannot be empty".to_string(),
            ));
        }

        // Length validation - prevent excessively long expressions
        if expr.len() > 1000 {
            return Err(ExpressionError::ParseError(
                "Expression too long (max 1000 characters)".to_string(),
            ));
        }

        // Character validation - allow only safe characters for mathematical expressions
        let valid_chars = expr.chars().all(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(
                    c,
                    '+' | '-' | '*' | '/' | '(' | ')' | '.' | '_' | ',' | '<' | '>' | '='
                )
        });

        if !valid_chars {
            // Find the actual invalid character
            let invalid_char = expr.chars().find(|c| {
                !(c.is_alphanumeric()
                    || c.is_whitespace()
                    || matches!(
                        c,
                        '+' | '-' | '*' | '/' | '(' | ')' | '.' | '_' | ',' | '<' | '>' | '='
                    ))
            });

            match invalid_char {
                Some(ch) => {
                    return Err(ExpressionError::ParseError(format!(
                        "Expression contains invalid character: '{}'",
                        ch
                    )));
                }
                None => {
                    // This should never happen if valid_chars is false, but handle gracefully
                    return Err(ExpressionError::ParseError(
                        "Expression validation failed - no invalid character found".to_string(),
                    ));
                }
            }
        }

        // Syntax validation - basic structure checks
        self.validate_expression_syntax(expr)?;

        // Division by zero prevention - check for literal division by zero
        self.validate_division_by_zero(expr)?;

        // Function validation - ensure only supported functions are used
        self.validate_supported_functions(expr)?;

        // Variable validation - ensure proper variable naming
        self.validate_variable_syntax(expr)?;

        // Parse validation - test if the expression can be parsed by the eval crate
        self.validate_eval_parsing(expr)?;

        Ok(())
    }

    /// Validate expression syntax structure
    fn validate_expression_syntax(&self, expr: &str) -> Result<(), ExpressionError> {
        // Check for balanced parentheses
        let mut paren_count = 0;
        for ch in expr.chars() {
            match ch {
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                _ => {}
            }
            if paren_count < 0 {
                return Err(ExpressionError::ParseError(
                    "Unbalanced parentheses - closing before opening".to_string(),
                ));
            }
        }

        if paren_count != 0 {
            return Err(ExpressionError::ParseError(
                "Unbalanced parentheses".to_string(),
            ));
        }

        // Check for invalid operator sequences
        let invalid_patterns = vec![
            "++", "--", "**", "//", // Double operators
            "+-", "-+", "*-", "/*", // Invalid operator combinations
        ];

        for pattern in invalid_patterns {
            if expr.contains(pattern) {
                return Err(ExpressionError::ParseError(format!(
                    "Invalid operator sequence: '{}'",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Validate division by zero prevention
    fn validate_division_by_zero(&self, expr: &str) -> Result<(), ExpressionError> {
        // Check for literal division by zero patterns
        let zero_div_patterns = vec!["/ 0", "/0 ", "/0.0", "/ 0.0", "/0.0 ", "/ 0.0 "];

        for pattern in zero_div_patterns {
            if expr.contains(pattern) {
                return Err(ExpressionError::DivisionByZero(format!(
                    "Literal division by zero detected: '{}'",
                    pattern.trim()
                )));
            }
        }

        // More sophisticated check - look for division followed by zero or variable that could be zero
        let tokens: Vec<&str> = expr.split_whitespace().collect();
        for i in 0..tokens.len() {
            if tokens[i] == "/" && i + 1 < tokens.len() {
                let next_token = tokens[i + 1];
                if next_token == "0" || next_token == "0.0" {
                    return Err(ExpressionError::DivisionByZero(
                        "Division by literal zero".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate that only supported mathematical functions are used
    fn validate_supported_functions(&self, expr: &str) -> Result<(), ExpressionError> {
        // List of supported mathematical functions
        let supported_functions = vec![
            "max", "min", "abs", "sqrt", "pow", "sin", "cos", "tan", "log", "ln", "exp", "floor",
            "ceil", "round",
        ];

        // Find all function calls (patterns like "function_name(")
        let function_pattern = regex::Regex::new(r"(\w+)\s*\(").unwrap();

        for cap in function_pattern.captures_iter(expr) {
            if let Some(func_name) = cap.get(1) {
                let func_name = func_name.as_str();
                if !supported_functions.contains(&func_name) {
                    return Err(ExpressionError::ParseError(format!(
                        "Unsupported function: '{}'",
                        func_name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate variable naming syntax
    fn validate_variable_syntax(&self, expr: &str) -> Result<(), ExpressionError> {
        // Validate local.variable syntax
        let local_var_pattern = regex::Regex::new(r"local\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for cap in local_var_pattern.captures_iter(expr) {
            if let Some(var_name) = cap.get(1) {
                let var_name = var_name.as_str();
                if var_name.is_empty() || var_name.starts_with(char::is_numeric) {
                    return Err(ExpressionError::ParseError(format!(
                        "Invalid local variable name: '{}'",
                        var_name
                    )));
                }
            }
        }

        // Validate global.variable syntax
        let global_var_pattern = regex::Regex::new(r"global\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for cap in global_var_pattern.captures_iter(expr) {
            if let Some(var_name) = cap.get(1) {
                let var_name = var_name.as_str();
                if var_name.is_empty() || var_name.starts_with(char::is_numeric) {
                    return Err(ExpressionError::ParseError(format!(
                        "Invalid global variable name: '{}'",
                        var_name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate that expression can be parsed by the eval crate
    fn validate_eval_parsing(&self, expr: &str) -> Result<(), ExpressionError> {
        // Convert syntax for eval crate
        let mut converted = expr.to_string();
        converted = converted.replace("local.", "local_");
        converted = converted.replace("global.", "global_");

        // Simple validation: just try to create the expression object
        // If it panics or fails, the expression is invalid
        // We use catch_unwind to handle panics gracefully
        let result = std::panic::catch_unwind(|| {
            let _expr_obj = Expr::new(&converted);
        });

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(ExpressionError::ParseError(
                "Invalid expression syntax - cannot be parsed by expression engine".to_string(),
            )),
        }
    }

    /// Extract variables from expression
    fn extract_variables(expr: &str, prefix: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let parts = expr.split_whitespace();

        for part in parts {
            if part.starts_with(prefix) {
                // Remove prefix and any trailing punctuation
                let var = part
                    .strip_prefix(prefix)
                    .unwrap_or("")
                    .trim_end_matches(|c| {
                        c == ')' || c == ',' || c == '+' || c == '-' || c == '*' || c == '/'
                    })
                    .to_string();
                if !var.is_empty() && !vars.contains(&var) {
                    vars.push(var);
                }
            }
        }
        vars
    }
}

impl<T> Expression<T> for SimpleExpression<T>
where
    T: Clone + Send + Sync + 'static + From<f64>,
{
    fn evaluate(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Result<T, ExpressionError> {
        // Convert syntax: local. -> local_, global. -> global_
        let mut converted = self.expression.clone();
        converted = converted.replace("local.", "local_");
        converted = converted.replace("global.", "global_");

        // Build eval expression with variables
        let mut expr = Expr::new(&converted);

        // Add variables
        for (name, value) in local_vars {
            expr = expr.value(format!("local_{}", name), *value);
        }
        for (name, value) in global_vars {
            expr = expr.value(format!("global_{}", name), *value);
        }

        // Evaluate
        match expr.exec() {
            Ok(result) => result
                .as_f64()
                .map(T::from)
                .ok_or_else(|| ExpressionError::ParseError("Not a number".to_string())),
            Err(e) => Err(ExpressionError::ParseError(format!("Eval error: {}", e))),
        }
    }

    fn required_local_vars(&self) -> Vec<String> {
        Self::extract_variables(&self.expression, "local.")
    }

    fn required_global_vars(&self) -> Vec<String> {
        Self::extract_variables(&self.expression, "global.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression_creation() {
        let expr = SimpleExpression::<f64>::new("local.cpu + global.memory").unwrap();
        assert_eq!(expr.expression, "local.cpu + global.memory");
    }

    #[test]
    fn test_basic_arithmetic() {
        let expr = SimpleExpression::<f64>::new("local.cpu").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 75.0);
        let global_vars = HashMap::new();

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 75.0);
    }

    #[test]
    fn test_division_by_zero_runtime() {
        let expr = SimpleExpression::<f64>::new("local.a / local.b").unwrap();
        let mut local_vars = HashMap::new();
        let global_vars = HashMap::new();

        // Test with b = 0 (runtime division by zero)
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 0.0);

        let result = expr.evaluate(&local_vars, &global_vars);
        // eval crate will handle division by zero at runtime
        assert!(result.is_err());
    }

    #[test]
    fn test_undefined_variable_error() {
        let expr = SimpleExpression::<f64>::new("local.missing + local.cpu").unwrap();
        let local_vars = HashMap::new(); // missing "missing" and "cpu"
        let global_vars = HashMap::new();

        let result = expr.evaluate(&local_vars, &global_vars);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExpressionError::ParseError(_)
        ));
    }

    #[test]
    fn test_basic_arithmetic_addition() {
        let expr = SimpleExpression::<f64>::new("local.a + local.b").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 20.0);
        let global_vars = HashMap::new();

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 30.0);
    }

    #[test]
    fn test_basic_arithmetic_multiplication() {
        let expr = SimpleExpression::<f64>::new("local.a * local.b").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 5.0);
        local_vars.insert("b".to_string(), 4.0);
        let global_vars = HashMap::new();

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_math_functions_max_simple() {
        let expr = SimpleExpression::<f64>::new("max(local.a, local.b)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 25.0);
        let global_vars = HashMap::new();

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_math_functions_max() {
        let expr = SimpleExpression::<f64>::new("max(local.a, local.b)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 25.0);
        let global_vars = HashMap::new();

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_mixed_local_global_arithmetic() {
        let expr = SimpleExpression::<f64>::new("local.cpu + global.memory").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 60.0);
        let mut global_vars = HashMap::new();
        global_vars.insert("memory".to_string(), 40.0);

        let result: f64 = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 100.0);
    }
}
