//! Expression evaluator
//!
//! Evaluates parsed expressions against variable values.

use crate::expression::types::{BinaryOp, Expr, Literal, UnaryOp};
use std::collections::{HashMap, VecDeque};

/// Error type for evaluation errors
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum EvalError {
    #[error("Variable not found: {0}")]
    UndefinedVariable(String),
    #[error("Unknown function: {0}")]
    UnknownFunction(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Invalid argument count")]
    InvalidArgCount,
}

/// Time window data structure
#[derive(Debug, Clone)]
pub struct TimeWindow {
    values: VecDeque<(u64, f64)>, // (timestamp, value)
    window_ms: u64,
}

impl TimeWindow {
    /// Create a new time window with the specified duration in milliseconds
    pub fn new(window_ms: u64) -> Self {
        Self {
            values: VecDeque::new(),
            window_ms,
        }
    }

    /// Add a value to the time window with the current timestamp
    pub fn add(&mut self, value: f64, timestamp: u64) {
        // Add the new value
        self.values.push_back((timestamp, value));
        
        // Remove values that are outside the window
        let cutoff_time = timestamp.saturating_sub(self.window_ms);
        while let Some(&(time, _)) = self.values.front() {
            if time < cutoff_time {
                self.values.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate the average of values in the window
    pub fn avg(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            let sum: f64 = self.values.iter().map(|(_, v)| v).sum();
            sum / self.values.len() as f64
        }
    }

    /// Find the maximum value in the window
    pub fn max(&self) -> f64 {
        self.values
            .iter()
            .map(|(_, v)| *v)
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0) // Return 0 if no values
    }

    /// Calculate the sum of values in the window
    pub fn sum(&self) -> f64 {
        self.values.iter().map(|(_, v)| v).sum()
    }

    /// Count values that satisfy a predicate in the window
    pub fn count(&self, predicate: impl Fn(f64) -> bool) -> usize {
        self.values.iter().filter(|(_, v)| predicate(*v)).count()
    }
}

/// Expression evaluator
pub struct Evaluator<'a> {
    values: &'a HashMap<String, f64>,
    windows: &'a mut HashMap<String, TimeWindow>,
    current_time: u64,
}

impl<'a> Evaluator<'a> {
    /// Create a new evaluator with the given variable values and time windows
    pub fn new(
        values: &'a HashMap<String, f64>,
        windows: &'a mut HashMap<String, TimeWindow>,
        current_time: u64,
    ) -> Self {
        Self {
            values,
            windows,
            current_time,
        }
    }

    /// Evaluate an expression and return the result
    pub fn evaluate(&mut self, expr: &Expr) -> Result<f64, EvalError> {
        match expr {
            Expr::Variable(name) => {
                self.values
                    .get(name)
                    .copied()
                    .ok_or_else(|| EvalError::UndefinedVariable(name.clone()))
            }
            Expr::Literal(lit) => match lit {
                Literal::Float(f) => Ok(*f),
                Literal::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            },
            Expr::Binary(left, op, right) => self.eval_binary(left, op.clone(), right),
            Expr::Unary(op, expr) => self.eval_unary(op.clone(), expr),
            Expr::Func(name, args) => self.eval_function(name, args),
            Expr::TimeWindow(name, window_ms) => self.eval_time_window(name, *window_ms),
        }
    }

    /// Evaluate a binary operation
    fn eval_binary(&mut self, left: &Expr, op: BinaryOp, right: &Expr) -> Result<f64, EvalError> {
        let left_val = self.evaluate(left)?;
        let right_val = self.evaluate(right)?;

        match op {
            BinaryOp::Add => Ok(left_val + right_val),
            BinaryOp::Sub => Ok(left_val - right_val),
            BinaryOp::Mul => Ok(left_val * right_val),
            BinaryOp::Div => {
                if right_val == 0.0 {
                    Ok(0.0) // Return 0 for division by zero to prevent crashes
                } else {
                    Ok(left_val / right_val)
                }
            }
            BinaryOp::Pow => Ok(left_val.powf(right_val)),
            BinaryOp::Eq => Ok(if left_val == right_val { 1.0 } else { 0.0 }),
            BinaryOp::Ne => Ok(if left_val != right_val { 1.0 } else { 0.0 }),
            BinaryOp::Gt => Ok(if left_val > right_val { 1.0 } else { 0.0 }),
            BinaryOp::Gte => Ok(if left_val >= right_val { 1.0 } else { 0.0 }),
            BinaryOp::Lt => Ok(if left_val < right_val { 1.0 } else { 0.0 }),
            BinaryOp::Lte => Ok(if left_val <= right_val { 1.0 } else { 0.0 }),
            BinaryOp::And => Ok(if left_val != 0.0 && right_val != 0.0 { 1.0 } else { 0.0 }),
            BinaryOp::Or => Ok(if left_val != 0.0 || right_val != 0.0 { 1.0 } else { 0.0 }),
        }
    }

    /// Evaluate a unary operation
    fn eval_unary(&mut self, op: UnaryOp, expr: &Expr) -> Result<f64, EvalError> {
        let value = self.evaluate(expr)?;

        match op {
            UnaryOp::Neg => Ok(-value),
            UnaryOp::Not => Ok(if value == 0.0 { 1.0 } else { 0.0 }),
        }
    }

    /// Evaluate a function call
    fn eval_function(&mut self, name: &str, args: &[Expr]) -> Result<f64, EvalError> {
        match name {
            // Math functions
            "abs" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.abs())
            }
            "sqrt" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.sqrt())
            }
            "pow" => {
                if args.len() != 2 {
                    return Err(EvalError::InvalidArgCount);
                }
                let base = self.evaluate(&args[0])?;
                let exp = self.evaluate(&args[1])?;
                Ok(base.powf(exp))
            }
            "round" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.round())
            }
            "floor" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.floor())
            }
            "ceil" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.ceil())
            }
            "max" => {
                if args.len() < 2 {
                    return Err(EvalError::InvalidArgCount);
                }
                let mut max_val = f64::NEG_INFINITY;
                for arg in args {
                    let val = self.evaluate(arg)?;
                    if val > max_val {
                        max_val = val;
                    }
                }
                Ok(max_val)
            }
            "min" => {
                if args.len() < 2 {
                    return Err(EvalError::InvalidArgCount);
                }
                let mut min_val = f64::INFINITY;
                for arg in args {
                    let val = self.evaluate(arg)?;
                    if val < min_val {
                        min_val = val;
                    }
                }
                Ok(min_val)
            }
            "sin" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.sin())
            }
            "cos" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.cos())
            }
            "tan" => {
                if args.len() != 1 {
                    return Err(EvalError::InvalidArgCount);
                }
                let val = self.evaluate(&args[0])?;
                Ok(val.tan())
            }
            _ => Err(EvalError::UnknownFunction(name.to_string())),
        }
    }

    /// Evaluate a time window expression
    fn eval_time_window(&mut self, name: &str, window_ms: u64) -> Result<f64, EvalError> {
        // Get or create the time window
        let window = self
            .windows
            .entry(name.to_string())
            .or_insert_with(|| TimeWindow::new(window_ms));

        // If we have a current value for this variable, add it to the window
        if let Some(&value) = self.values.get(name) {
            window.add(value, self.current_time);
        }

        // Return the average for now - in a real implementation, this would depend on context
        Ok(window.avg())
    }
}