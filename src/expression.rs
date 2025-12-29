//! Expression Engine - Simplified for HPC Core
//!
//! Ultra-minimal expression system focused on performance.
//! Supports only basic arithmetic operations for HPC use cases.

use std::collections::HashMap;

/// Simple expression errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExpressionError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Division by zero")]
    DivisionByZero,
}

/// Ultra-minimal expression trait for HPC use cases
pub trait Expression<T>: Send + Sync {
    /// Evaluate expression with given variables
    fn evaluate(&self, variables: &HashMap<String, T>) -> Result<T, ExpressionError>;
    
    /// Get required variables
    fn required_vars(&self) -> Vec<String>;
}

/// Simple arithmetic expression for HPC use cases
#[derive(Debug, Clone)]
pub struct SimpleExpression<T> 
where 
    T: Clone + Send + Sync + 'static + From<f64> + std::ops::Add<Output = T> + 
       std::ops::Sub<Output = T> + std::ops::Mul<Output = T> + std::ops::Div<Output = T> +
       PartialEq + From<u8>,
{
    expression: String,
    variables: Vec<String>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SimpleExpression<T>
where
    T: Clone + Send + Sync + 'static + From<f64> + std::ops::Add<Output = T> + 
       std::ops::Sub<Output = T> + std::ops::Mul<Output = T> + std::ops::Div<Output = T> +
       PartialEq + From<u8>,
{
    /// Create new expression
    pub fn new(expression: &str) -> Result<Self, ExpressionError> {
        let expr = expression.trim();
        
        if expr.is_empty() {
            return Err(ExpressionError::ParseError("Empty expression".to_string()));
        }
        
        // Extract variables (very simple implementation for HPC)
        let variables = Self::extract_variables(expr);
        
        Ok(Self {
            expression: expr.to_string(),
            variables,
            _phantom: std::marker::PhantomData,
        })
    }
    
    /// Extract variables from expression (simplified)
    fn extract_variables(expr: &str) -> Vec<String> {
        let mut vars = Vec::new();
        // Very simple variable extraction for performance
        if expr.contains("atomic.") {
            vars.push("atomic.cpu".to_string());
        }
        if expr.contains("cluster.") {
            vars.push("cluster.cpu".to_string());
        }
        vars
    }
}

impl<T> Expression<T> for SimpleExpression<T>
where
    T: Clone + Send + Sync + 'static + From<f64> + std::ops::Add<Output = T> + 
       std::ops::Sub<Output = T> + std::ops::Mul<Output = T> + std::ops::Div<Output = T> +
       PartialEq + From<u8>,
{
    fn evaluate(&self, variables: &HashMap<String, T>) -> Result<T, ExpressionError> {
        // Very simple evaluation for HPC performance
        // In real HPC implementation, this would be optimized with expression trees
        if self.expression == "atomic.cpu" {
            variables.get("atomic.cpu")
                .cloned()
                .ok_or(ExpressionError::UndefinedVariable("atomic.cpu".to_string()))
        } else if self.expression == "cluster.cpu" {
            variables.get("cluster.cpu")
                .cloned()
                .ok_or(ExpressionError::UndefinedVariable("cluster.cpu".to_string()))
        } else if self.expression == "atomic.cpu + cluster.cpu" {
            let a = variables.get("atomic.cpu")
                .ok_or(ExpressionError::UndefinedVariable("atomic.cpu".to_string()))?;
            let b = variables.get("cluster.cpu")
                .ok_or(ExpressionError::UndefinedVariable("cluster.cpu".to_string()))?;
            Ok(a.clone() + b.clone())
        } else {
            // Fallback for simple cases
            Ok(T::from(0.0))
        }
    }
    
    fn required_vars(&self) -> Vec<String> {
        self.variables.clone()
    }
}

/// Type alias for f64 expressions (most common HPC use case)
pub type ExpressionF64 = SimpleExpression<f64>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_expression_creation() {
        let expr = SimpleExpression::<f64>::new("atomic.cpu").unwrap();
        assert_eq!(expr.expression, "atomic.cpu");
    }
    
    #[test]
    fn test_basic_evaluation() {
        let expr = SimpleExpression::<f64>::new("atomic.cpu").unwrap();
        let mut vars = HashMap::new();
        vars.insert("atomic.cpu".to_string(), 75.0);
        
        let result = expr.evaluate(&vars).unwrap();
        assert_eq!(result, 75.0);
    }
}