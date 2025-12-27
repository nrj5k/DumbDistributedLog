//! Expression Engine - Migrated to evalexpr
//!
//! Ultra-minimal expression system with 3-method trait.
//! Supports basic arithmetic with atomic/cluster variables.

use evalexpr::*;
use std::collections::HashMap;

/// Expression errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExpressionError {
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    #[error("Division by zero in: {0}")]
    DivisionByZero(String),
}

/// Ultra-minimal expression trait for maximum flexibility
pub trait Expression<T>: Send + Sync {
    fn convert_syntax(&self, expr: &str) -> Result<String, ExpressionError> {
        let mut converted = expr.to_string();
        converted = converted.replace("atomic.", "atomic_");
        converted = converted.replace("cluster.", "cluster_");
        Ok(converted)
    }
    /// Evaluate expression with given variable contexts
    fn evaluate(
        &self,
        atomic_vars: &HashMap<String, f64>,
        cluster_vars: &HashMap<String, f64>,
    ) -> Result<T, ExpressionError>;

    /// Get required atomic variables
    fn required_atomic_vars(&self) -> Vec<String>;

    /// Get required cluster variables
    fn required_cluster_vars(&self) -> Vec<String>;
}

/// Type alias for f64 expressions (most common use case)
pub type ExpressionF64 = SimpleExpression<f64>;

/// Simple expression implementation using evalexpr
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

        let simple_expr = Self {
            expression: expr.to_string(),
            _phantom: std::marker::PhantomData,
        };

        simple_expr.validate_expression()?;

        Ok(simple_expr)
    }

    /// Comprehensive expression validation with detailed error reporting
    fn validate_expression(&self) -> Result<(), ExpressionError> {
        let expr = self.expression.trim();

        if expr.is_empty() {
            return Err(ExpressionError::ParseError(
                "Expression cannot be empty".to_string(),
            ));
        }

        if expr.len() > 1000 {
            return Err(ExpressionError::ParseError(
                "Expression too long (max 1000 characters)".to_string(),
            ));
        }

        let valid_chars = expr.chars().all(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(
                    c,
                    '+' | '-' | '*' | '/' | '(' | ')' | '.' | '_' | ',' | '<' | '>' | '='
                )
        });

        if !valid_chars {
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
                    return Err(ExpressionError::ParseError(
                        "Expression validation failed - no invalid character found".to_string(),
                    ));
                }
            }
        }

        self.validate_expression_syntax(expr)?;
        self.validate_division_by_zero(expr)?;
        self.validate_supported_functions(expr)?;
        self.validate_variable_syntax(expr)?;
        self.validate_evalexpr_parsing(expr)?;

        Ok(())
    }

    fn validate_expression_syntax(&self, expr: &str) -> Result<(), ExpressionError> {
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

        let invalid_patterns = vec!["++", "--", "**", "//", "+-", "-+", "*-", "/*"];

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

    fn validate_division_by_zero(&self, expr: &str) -> Result<(), ExpressionError> {
        let zero_div_patterns = vec!["/ 0", "/0 ", "/0.0", "/ 0.0", "/0.0 ", "/ 0.0 "];

        for pattern in zero_div_patterns {
            if expr.contains(pattern) {
                return Err(ExpressionError::DivisionByZero(format!(
                    "Literal division by zero detected: '{}'",
                    pattern.trim()
                )));
            }
        }

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

    fn validate_supported_functions(&self, expr: &str) -> Result<(), ExpressionError> {
        let supported_functions = vec![
            "max", "min", "abs", "sqrt", "pow", "sin", "cos", "tan", "log", "ln", "exp", "floor",
            "ceil", "round",
        ];

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

    fn validate_variable_syntax(&self, expr: &str) -> Result<(), ExpressionError> {
        let atomic_var_pattern = regex::Regex::new(r"atomic\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for cap in atomic_var_pattern.captures_iter(expr) {
            if let Some(var_name) = cap.get(1) {
                let var_name = var_name.as_str();
                if var_name.is_empty() || var_name.starts_with(char::is_numeric) {
                    return Err(ExpressionError::ParseError(format!(
                        "Invalid atomic variable name: '{}'",
                        var_name
                    )));
                }
            }
        }

        let cluster_var_pattern = regex::Regex::new(r"cluster\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for cap in cluster_var_pattern.captures_iter(expr) {
            if let Some(var_name) = cap.get(1) {
                let var_name = var_name.as_str();
                if var_name.is_empty() || var_name.starts_with(char::is_numeric) {
                    return Err(ExpressionError::ParseError(format!(
                        "Invalid cluster variable name: '{}'",
                        var_name
                    )));
                }
            }
        }

        Ok(())
    }

    fn validate_evalexpr_parsing(&self, expr: &str) -> Result<(), ExpressionError> {
        let mut converted = expr.to_string();
        converted = converted.replace("atomic.", "atomic_");
        converted = converted.replace("cluster.", "cluster_");

        match build_operator_tree::<DefaultNumericTypes>(&converted) {
            Ok(_) => Ok(()),
            Err(e) => Err(ExpressionError::ParseError(format!(
                "Invalid expression syntax - cannot be parsed by evalexpr: {}",
                e
            ))),
        }
    }

    fn extract_variables(expr: &str, prefix: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let parts = expr.split_whitespace();

        for part in parts {
            if part.starts_with(prefix) {
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
        atomic_vars: &HashMap<String, f64>,
        cluster_vars: &HashMap<String, f64>,
    ) -> Result<T, ExpressionError> {
        let mut converted = self.expression.clone();
        converted = converted.replace("atomic.", "atomic_");
        converted = converted.replace("cluster.", "cluster_");

        let mut context = HashMapContext::<DefaultNumericTypes>::new();

        for (name, value) in atomic_vars {
            let _ = context.set_value(format!("atomic_{}", name), Value::from_float(*value));
        }
        for (name, value) in cluster_vars {
            let _ = context.set_value(format!("cluster_{}", name), Value::from_float(*value));
        }

        match eval_with_context(&converted, &context) {
            Ok(result) => result
                .as_float()
                .map(|f| T::from(f))
                .map_err(|e| ExpressionError::ParseError(format!("Not a number: {}", e))),
            Err(e) => Err(ExpressionError::ParseError(format!("Evalexpr error: {}", e))),
        }
    }

    fn required_atomic_vars(&self) -> Vec<String> {
        Self::extract_variables(&self.expression, "atomic.")
    }

    fn required_cluster_vars(&self) -> Vec<String> {
        Self::extract_variables(&self.expression, "cluster.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression_creation() {
        let expr = SimpleExpression::<f64>::new("atomic.cpu + cluster.memory").unwrap();
        assert_eq!(expr.expression, "atomic.cpu + cluster.memory");
    }

    #[test]
    fn test_basic_arithmetic() {
        let expr = SimpleExpression::<f64>::new("atomic.cpu").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("cpu".to_string(), 75.0);
        let cluster_vars = HashMap::new();

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 75.0);
    }

    #[test]
    fn test_division_by_zero_runtime() {
        let expr = SimpleExpression::<f64>::new("atomic.a / atomic.b").unwrap();
        let mut atomic_vars = HashMap::new();
        let cluster_vars = HashMap::new();

        atomic_vars.insert("a".to_string(), 10.0);
        atomic_vars.insert("b".to_string(), 0.0);

        let _result = expr.evaluate(&atomic_vars, &cluster_vars);
    }

    #[test]
    fn test_undefined_variable_error() {
        let expr = SimpleExpression::<f64>::new("atomic.missing + atomic.cpu").unwrap();
        let atomic_vars = HashMap::new();
        let cluster_vars = HashMap::new();

        let result = expr.evaluate(&atomic_vars, &cluster_vars);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExpressionError::ParseError(_)));
    }

    #[test]
    fn test_basic_arithmetic_addition() {
        let expr = SimpleExpression::<f64>::new("atomic.a + atomic.b").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("a".to_string(), 10.0);
        atomic_vars.insert("b".to_string(), 20.0);
        let cluster_vars = HashMap::new();

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 30.0);
    }

    #[test]
    fn test_basic_arithmetic_multiplication() {
        let expr = SimpleExpression::<f64>::new("atomic.a * atomic.b").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("a".to_string(), 5.0);
        atomic_vars.insert("b".to_string(), 4.0);
        let cluster_vars = HashMap::new();

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_math_functions_max_simple() {
        let expr = SimpleExpression::<f64>::new("max(atomic.a, atomic.b)").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("a".to_string(), 10.0);
        atomic_vars.insert("b".to_string(), 25.0);
        let cluster_vars = HashMap::new();

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_math_functions_max() {
        let expr = SimpleExpression::<f64>::new("max(atomic.a, atomic.b)").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("a".to_string(), 10.0);
        atomic_vars.insert("b".to_string(), 25.0);
        let cluster_vars = HashMap::new();

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_mixed_atomic_cluster_arithmetic() {
        let expr = SimpleExpression::<f64>::new("atomic.cpu + cluster.memory").unwrap();
        let mut atomic_vars = HashMap::new();
        atomic_vars.insert("cpu".to_string(), 60.0);
        let mut cluster_vars = HashMap::new();
        cluster_vars.insert("memory".to_string(), 40.0);

        let result: f64 = expr.evaluate(&atomic_vars, &cluster_vars).unwrap();
        assert_eq!(result, 100.0);
    }
}
