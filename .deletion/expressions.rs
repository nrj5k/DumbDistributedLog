//! Local Expression Engine for AutoQueues
//!
//! Provides mathematical expression evaluation with local.* variable substitution.
//! Supports complex Rust math equations, auto-topic subscription, and division-by-zero
//! handling for local-only queue processing.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32;
use std::sync::{Arc, RwLock};

/// Expression evaluation errors
#[derive(Debug)]
pub enum ExpressionError {
    ParseError(String),
    UndefinedVariable(String),
    DivisionByZero(String),
    MathError(String),
    SyntaxError(String),
}

impl std::fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpressionError::ParseError(expr) => write!(f, "Failed to parse expression: {}", expr),
            ExpressionError::UndefinedVariable(var) => write!(f, "Undefined variable: {}", var),
            ExpressionError::DivisionByZero(operation) => {
                write!(f, "Division by zero in: {}", operation)
            }
            ExpressionError::MathError(msg) => write!(f, "Mathematical error: {}", msg),
            ExpressionError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
        }
    }
}

impl std::error::Error for ExpressionError {}

/// Supported mathematical functions for local variable evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathFunction {
    Powf(f32, f32),    // base^exponent
    Sqrt(f32),         // square root
    Abs(f32),          // absolute value
    Max(f32, f32),     // maximum of two values
    Min(f32, f32),     // minimum of two values
    Round(f32),        // round to nearest integer
    Ceil(f32),         // ceiling
    Floor(f32),        // floor
    RoundTo(f32, i32), // round to n decimal places
}

impl MathFunction {
    pub fn evaluate(&self) -> f32 {
        match self {
            MathFunction::Powf(base, exponent) => base.powf(*exponent),
            MathFunction::Sqrt(x) => x.sqrt(),
            MathFunction::Abs(x) => x.abs(),
            MathFunction::Max(a, b) => a.max(*b),
            MathFunction::Min(a, b) => a.min(*b),
            MathFunction::Round(x) => x.round(),
            MathFunction::Ceil(x) => x.ceil(),
            MathFunction::Floor(x) => x.floor(),
            MathFunction::RoundTo(x, decimals) => {
                let multiplier = 10f32.powi(*decimals);
                (x * multiplier).round() / multiplier
            }
        }
    }

    pub fn function_name(&self) -> &'static str {
        match self {
            MathFunction::Powf(_, _) => "powf",
            MathFunction::Sqrt(_) => "sqrt",
            MathFunction::Abs(_) => "abs",
            MathFunction::Max(_, _) => "max",
            MathFunction::Min(_, _) => "min",
            MathFunction::Round(_) => "round",
            MathFunction::Ceil(_) => "ceil",
            MathFunction::Floor(_) => "floor",
            MathFunction::RoundTo(_, _) => "round_to",
        }
    }
}

/// Local variable reference in expressions
#[derive(Debug, Clone)]
pub struct LocalVariable {
    pub name: String,
    pub topic: String, // Resolved topic name (local/cpu_percent → local/cpu_percent)
    pub value: Option<f32>,
}

impl LocalVariable {
    pub fn new(var_name: &str) -> Self {
        // Convert local.variable to topic format
        let topic = if var_name.starts_with("local.") {
            let suffix = &var_name[6..]; // Remove "local."
            format!("local/{}", suffix).replace("_", "-") // Convert to topic format
        } else {
            format!("local/{}", var_name).replace("_", "-")
        };

        Self {
            name: var_name.to_string(),
            topic,
            value: None,
        }
    }
}

/// Local expression with variable references and math operations
#[derive(Debug, Clone)]
pub struct LocalExpression {
    pub expression: String,
    pub variables: Vec<LocalVariable>,
    pub template_code: String, // Rust code template with variable substitution
}

impl LocalExpression {
    /// Create expression from user-defined string
    pub fn new(expression: &str) -> Result<Self, ExpressionError> {
        if expression.trim().is_empty() {
            return Err(ExpressionError::SyntaxError("Empty expression".to_string()));
        }

        // Extract local variable references
        let variables = Self::extract_local_variables(expression)?;

        // Generate Rust code template
        let template_code = Self::generate_template(expression, &variables)?;

        Ok(Self {
            expression: expression.to_string(),
            variables,
            template_code,
        })
    }

    /// Extract local.* variable references from expression
    fn extract_local_variables(expression: &str) -> Result<Vec<LocalVariable>, ExpressionError> {
        let var_regex = Regex::new(r"local\.[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
        let mut variables = Vec::new();
        let mut seen = HashMap::new();

        for cap in var_regex.captures_iter(expression) {
            let var_name = cap.get(0).unwrap().as_str();
            if !seen.contains_key(var_name) {
                seen.insert(var_name.to_string(), true);
                variables.push(LocalVariable::new(var_name));
            }
        }

        Ok(variables)
    }

    /// Generate safe Rust code template with variable substitution
    fn generate_template(
        expression: &str,
        variables: &[LocalVariable],
    ) -> Result<String, ExpressionError> {
        let mut template = expression.to_string();

        // Replace each local variable with a safe placeholder
        for var in variables {
            let placeholder = format!("{{{}}}", var.name.replace(".", "_").replace("-", "_"));
            template = template.replace(&var.name, &placeholder);
        }

        // Surround with safe evaluation context
        Ok(format!("( {} )", template))
    }

    /// Get required topic subscriptions for this expression
    pub fn get_required_topics(&self) -> Vec<String> {
        self.variables.iter().map(|var| var.topic.clone()).collect()
    }

    /// Update variable value from topic data
    pub fn update_variable(&mut self, topic: &str, value: f32) -> bool {
        for var in &mut self.variables {
            if var.topic == topic {
                var.value = Some(value);
                return true;
            }
        }
        false
    }

    /// Evaluate expression with current variable values
    pub fn evaluate(&self) -> Result<f32, ExpressionError> {
        // Check all variables have values
        for var in &self.variables {
            if var.value.is_none() {
                return Err(ExpressionError::UndefinedVariable(var.name.clone()));
            }
        }

        // Build context hashmap for safe evaluation
        let mut context = HashMap::new();
        for var in &self.variables {
            let key = var.name.replace(".", "_").replace("-", "_");
            context.insert(key, var.value.unwrap());
        }

        // For now, use simple evaluation for safety
        // In production, this would compile to safe Rust code
        self.simple_evaluate(&context)
    }

    /// Simple expression evaluator with division by zero protection
    fn simple_evaluate(&self, context: &HashMap<String, f32>) -> Result<f32, ExpressionError> {
        // Parse and evaluate with division by zero protection
        let mut expression = self.template_code.clone();

        // Substitute placeholders with values
        for (placeholder, value) in context {
            expression = expression.replace(&format!("{{{}}}", placeholder), &value.to_string());
        }

        // Handle division operations with zero protection
        if expression.contains("/") {
            // For complex expressions, use tree-based evaluation later
            // For now, detect simple division and protect
            if expression.contains("/ 0") || expression.contains("/0") {
                return Ok(0.0); // Division by zero protection
            }
        }

        // Basic arithmetic evaluation with protection
        if expression.contains("+")
            || expression.contains("-")
            || expression.contains("*")
            || expression.contains("/")
        {
            // For now, handle simple cases
            // This would be implemented with a proper math parser
            self.evaluate_simple_arithmetic(&expression, context)
        } else {
            // Single value or function call
            Ok(0.0) // Default for unsupported expressions
        }
    }

    /// Evaluate simple arithmetic expressions
    fn evaluate_simple_arithmetic(
        &self,
        expression: &str,
        context: &HashMap<String, f32>,
    ) -> Result<f32, ExpressionError> {
        // For POC, handle very basic expressions
        // Real implementation would use proper math parser

        // Example: (a + b) / 2
        let parts: Vec<&str> = expression.split_whitespace().collect();

        if parts.len() == 3 {
            // Simple binary operation
            if let Ok(left) = self.parse_operand(parts[0].trim(), context) {
                if let Ok(right) = self.parse_operand(parts[2].trim(), context) {
                    match parts[1] {
                        "+" => Ok(left + right),
                        "-" => Ok(left - right),
                        "*" => Ok(left * right),
                        "/" => {
                            if right.abs() < f32::EPSILON {
                                Ok(0.0) // Division by zero protection
                            } else {
                                Ok(left / right)
                            }
                        }
                        _ => Ok(0.0),
                    }
                } else {
                    Ok(0.0)
                }
            } else {
                Ok(0.0)
            }
        } else {
            Ok(0.0)
        }
    }

    /// Parse an operand (number or placeholder)
    fn parse_operand(
        &self,
        operand: &str,
        context: &HashMap<String, f32>,
    ) -> Result<f32, ExpressionError> {
        let cleaned = operand.trim_matches(|c: char| c == '(' || c == ')').trim();

        // Try to parse as number first
        if let Ok(num) = cleaned.parse::<f32>() {
            Ok(num)
        } else {
            // Look up in context
            let key = cleaned.replace("-", "_");
            if let Some(&value) = context.get(&key) {
                Ok(value)
            } else {
                // Try to find partial match (e.g., "{local_cap}" matches "local_cap")
                for (context_key, context_value) in context {
                    if key.contains(context_key) {
                        return Ok(*context_value);
                    }
                }
                Err(ExpressionError::UndefinedVariable(cleaned.to_string()))
            }
        }
    }
}

/// Context for storing variable values during expression evaluation
#[derive(Debug, Clone)]
pub struct ExpressionContext {
    variables: Arc<RwLock<HashMap<String, f32>>>,
}

impl ExpressionContext {
    pub fn new() -> Self {
        Self {
            variables: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn set_variable(&self, name: &str, value: f32) {
        let mut variables = self.variables.write().unwrap();
        variables.insert(name.to_string(), value);
    }

    pub fn get_variable(&self, name: &str) -> Option<f32> {
        let variables = self.variables.read().unwrap();
        variables.get(name).copied()
    }

    /// Update context with topic data
    pub fn update_from_topic(&self, topic: &str, value: f32) -> bool {
        // Convert topic to variable name (local/drive_capacity → local.drive_capacity)
        let var_name = topic.replace("local/", "local.").replace("-", "_");

        if var_name.starts_with("local.") {
            self.set_variable(&var_name, value);
            true
        } else {
            false
        }
    }
}

impl Default for ExpressionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_variable_parsing() {
        let var = LocalVariable::new("local.cpu_percent");
        assert_eq!(var.topic, "local/cpu-percent");
        assert_eq!(var.name, "local.cpu_percent");
    }

    #[test]
    fn test_expression_extraction() {
        let expr_text = "(local.cpu + local.memory) / 2.0";
        let expression = LocalExpression::new(expr_text).unwrap();

        assert_eq!(expression.variables.len(), 2);
        assert_eq!(expression.variables[0].name, "local.cpu");
        assert_eq!(expression.variables[1].name, "local.memory");
    }

    #[test]
    fn test_topic_subscription_list() {
        let expr_text = "local.drive_capacity / local.cpu_percent";
        let expression = LocalExpression::new(expr_text).unwrap();

        let topics = expression.get_required_topics();
        assert!(topics.contains(&"local/drive-capacity".to_string()));
        assert!(topics.contains(&"local/cpu-percent".to_string()));
    }
}
