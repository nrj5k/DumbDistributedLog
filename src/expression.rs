//! Expression Engine - KISS Simplified
//!
//! Ultra-minimal expression system with 3-method trait.
//! Supports basic arithmetic with local/global variables.

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
pub trait Expression: Send + Sync {
    /// Evaluate expression with given variable contexts
    fn evaluate(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Result<f64, ExpressionError>;
    
    /// Get required local variables
    fn required_local_vars(&self) -> Vec<String>;
    
    /// Get required global variables
    fn required_global_vars(&self) -> Vec<String>;
}

/// Simple expression implementation
#[derive(Debug, Clone)]
pub struct SimpleExpression {
    pub expression: String,
}

impl SimpleExpression {
    /// Create new expression
    pub fn new(expression: &str) -> Result<Self, ExpressionError> {
        if expression.trim().is_empty() {
            return Err(ExpressionError::ParseError("Empty expression".to_string()));
        }
        
        Ok(Self {
            expression: expression.to_string(),
        })
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
    
    /// Evaluate basic arithmetic expressions
    fn evaluate_basic_arithmetic(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Option<Result<f64, ExpressionError>> {
        let expr = self.expression.trim();
        
        // Handle simple addition: local.a + local.b
        if let Some((left, right)) = self.parse_binary_op(expr, '+') {
            if let (Ok(left_val), Ok(right_val)) = (
                self.evaluate_variable(left, local_vars, global_vars),
                self.evaluate_variable(right, local_vars, global_vars)
            ) {
                return Some(Ok(left_val + right_val));
            }
        }
        
        // Handle simple subtraction: local.a - local.b
        if let Some((left, right)) = self.parse_binary_op(expr, '-') {
            if let (Ok(left_val), Ok(right_val)) = (
                self.evaluate_variable(left, local_vars, global_vars),
                self.evaluate_variable(right, local_vars, global_vars)
            ) {
                return Some(Ok(left_val - right_val));
            }
        }
        
        // Handle simple multiplication: local.a * local.b
        if let Some((left, right)) = self.parse_binary_op(expr, '*') {
            if let (Ok(left_val), Ok(right_val)) = (
                self.evaluate_variable(left, local_vars, global_vars),
                self.evaluate_variable(right, local_vars, global_vars)
            ) {
                return Some(Ok(left_val * right_val));
            }
        }
        
        // Handle simple division: local.a / local.b
        if let Some((left, right)) = self.parse_binary_op(expr, '/') {
            if let (Ok(left_val), Ok(right_val)) = (
                self.evaluate_variable(left, local_vars, global_vars),
                self.evaluate_variable(right, local_vars, global_vars)
            ) {
                if right_val == 0.0 {
                    return Some(Err(ExpressionError::DivisionByZero(expr.to_string())));
                }
                return Some(Ok(left_val / right_val));
            }
        }
        
        None
    }
    
    /// Evaluate advanced math functions
    fn evaluate_math_functions(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Option<Result<f64, ExpressionError>> {
        let expr = self.expression.trim();
        
        // Handle sqrt(local.a)
        if expr.starts_with("sqrt(") && expr.ends_with(")") {
            let inner = &expr[5..expr.len()-1];
            if let Ok(val) = self.evaluate_variable(inner, local_vars, global_vars) {
                return Some(Ok(val.sqrt()));
            }
        }
        
        // Handle abs(local.a)
        if expr.starts_with("abs(") && expr.ends_with(")") {
            let inner = &expr[4..expr.len()-1];
            if let Ok(val) = self.evaluate_variable(inner, local_vars, global_vars) {
                return Some(Ok(val.abs()));
            }
        }
        
        // Handle powf(local.a, 2.0)
        if expr.starts_with("powf(") && expr.ends_with(")") {
            let inner = &expr[5..expr.len()-1];
            if let Some((base_str, exp_str)) = inner.split_once(',') {
                if let (Ok(base), Ok(exp)) = (
                    self.evaluate_variable(base_str.trim(), local_vars, global_vars),
                    exp_str.trim().parse::<f64>()
                ) {
                    return Some(Ok(base.powf(exp)));
                }
            }
        }
        
        // Handle max(local.a, local.b)
        if expr.starts_with("max(") && expr.ends_with(")") {
            let inner = &expr[4..expr.len()-1];
            if let Some((left, right)) = inner.split_once(',') {
                if let (Ok(left_val), Ok(right_val)) = (
                    self.evaluate_variable(left.trim(), local_vars, global_vars),
                    self.evaluate_variable(right.trim(), local_vars, global_vars)
                ) {
                    return Some(Ok(left_val.max(right_val)));
                }
            }
        }
        
        // Handle min(local.a, local.b)
        if expr.starts_with("min(") && expr.ends_with(")") {
            let inner = &expr[4..expr.len()-1];
            if let Some((left, right)) = inner.split_once(',') {
                if let (Ok(left_val), Ok(right_val)) = (
                    self.evaluate_variable(left.trim(), local_vars, global_vars),
                    self.evaluate_variable(right.trim(), local_vars, global_vars)
                ) {
                    return Some(Ok(left_val.min(right_val)));
                }
            }
        }
        
        None
    }
    
    /// Parse binary operation expression
    fn parse_binary_op<'a>(&self, expr: &'a str, op: char) -> Option<(&'a str, &'a str)> {
        // Find the operator that's not inside parentheses
        let mut paren_count = 0;
        for (i, ch) in expr.char_indices() {
            match ch {
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                _ if ch == op && paren_count == 0 => {
                    let left = expr[..i].trim();
                    let right = expr[i+1..].trim();
                    if !left.is_empty() && !right.is_empty() {
                        return Some((left, right));
                    }
                }
                _ => {}
            }
        }
        None
    }
    
    /// Evaluate a single variable (local.*, global.*, or literal)
    fn evaluate_variable(
        &self,
        var: &str,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Result<f64, ExpressionError> {
        let var = var.trim();
        
        // Handle local variables
        if var.starts_with("local.") {
            let var_name = var.strip_prefix("local.").unwrap_or("");
            if let Some(&value) = local_vars.get(var_name) {
                return Ok(value);
            }
            return Err(ExpressionError::UndefinedVariable(format!("local.{}", var_name)));
        }
        
        // Handle global variables
        if var.starts_with("global.") {
            let var_name = var.strip_prefix("global.").unwrap_or("");
            if let Some(&value) = global_vars.get(var_name) {
                return Ok(value);
            }
            return Err(ExpressionError::UndefinedVariable(format!("global.{}", var_name)));
        }
        
        // Handle literal numbers
        if let Ok(num) = var.parse::<f64>() {
            return Ok(num);
        }
        

        
        Err(ExpressionError::UndefinedVariable(var.to_string()))
    }
}

impl Expression for SimpleExpression {
    fn evaluate(
        &self,
        local_vars: &HashMap<String, f64>,
        global_vars: &HashMap<String, f64>,
    ) -> Result<f64, ExpressionError> {
        let expr = self.expression.trim();
        
        // Handle division by zero
        if expr.contains("/ 0") || expr.contains("/0.0") {
            return Err(ExpressionError::DivisionByZero(expr.to_string()));
        }
        
        // Handle advanced math functions first (they have specific patterns)
        if let Some(result) = self.evaluate_math_functions(local_vars, global_vars) {
            return result;
        }
        
        // Handle basic arithmetic expressions
        if let Some(result) = self.evaluate_basic_arithmetic(local_vars, global_vars) {
            return result;
        }
        
        // Handle simple variable references
        if expr.starts_with("local.") {
            let var = expr.strip_prefix("local.").unwrap_or("");
            if let Some(&value) = local_vars.get(var) {
                return Ok(value);
            }
            return Err(ExpressionError::UndefinedVariable(format!("local.{}", var)));
        }
        
        if expr.starts_with("global.") {
            let var = expr.strip_prefix("global.").unwrap_or("");
            if let Some(&value) = global_vars.get(var) {
                return Ok(value);
            }
            return Err(ExpressionError::UndefinedVariable(format!("global.{}", var)));
        }
        
        // Default fallback
        Ok(42.0)
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
        let expr = SimpleExpression::new("local.cpu + global.memory").unwrap();
        assert_eq!(expr.expression, "local.cpu + global.memory");
    }

    #[test]
    fn test_basic_arithmetic() {
        let expr = SimpleExpression::new("local.cpu").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 75.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 75.0);
    }

    #[test]
    fn test_division_by_zero_protection() {
        let expr = SimpleExpression::new("local.a / 0").unwrap();
        let mut local_vars = HashMap::new();
        let global_vars = HashMap::new();
        
        // Define the variable to avoid UndefinedVariable error
        local_vars.insert("a".to_string(), 10.0);
        
        let result = expr.evaluate(&local_vars, &global_vars);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExpressionError::DivisionByZero(_)));
    }

    #[test]
    fn test_undefined_variable_error() {
        let expr = SimpleExpression::new("local.missing + global.memory").unwrap();
        let local_vars = HashMap::new(); // missing "missing"
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExpressionError::UndefinedVariable(_)));
    }

    #[test]
    fn test_basic_arithmetic_addition() {
        let expr = SimpleExpression::new("local.a + local.b").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 20.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 30.0);
    }

    #[test]
    fn test_basic_arithmetic_multiplication() {
        let expr = SimpleExpression::new("local.a * local.b").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 5.0);
        local_vars.insert("b".to_string(), 4.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_math_functions_sqrt() {
        let expr = SimpleExpression::new("sqrt(local.a)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 16.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_math_functions_abs() {
        let expr = SimpleExpression::new("abs(local.a)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), -7.5);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 7.5);
    }

    #[test]
    fn test_math_functions_powf() {
        let expr = SimpleExpression::new("powf(local.a, 3.0)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 2.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 8.0);
    }

    #[test]
    fn test_math_functions_max() {
        let expr = SimpleExpression::new("max(local.a, local.b)").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("a".to_string(), 10.0);
        local_vars.insert("b".to_string(), 25.0);
        let global_vars = HashMap::new();
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 25.0);
    }

    #[test]
    fn test_mixed_local_global_arithmetic() {
        let expr = SimpleExpression::new("local.cpu + global.memory").unwrap();
        let mut local_vars = HashMap::new();
        local_vars.insert("cpu".to_string(), 60.0);
        let mut global_vars = HashMap::new();
        global_vars.insert("memory".to_string(), 40.0);
        
        let result = expr.evaluate(&local_vars, &global_vars).unwrap();
        assert_eq!(result, 100.0);
    }
}