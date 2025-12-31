//! Expression AST types
//!
//! Defines the abstract syntax tree nodes for the expression language.

/// Supported binary operations
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Pow,
    Eq, Ne, Gt, Gte, Lt, Lte,
    And, Or,
}

/// Unary operations  
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum UnaryOp {
    Neg, Not,
}

/// Literal values
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Float(f64),
    Bool(bool),
}

/// AST Node
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Variable(String),           // "cpu"
    Literal(Literal),           // 42.0 or true
    Binary(Box<Expr>, BinaryOp, Box<Expr>),  // a + b
    Unary(UnaryOp, Box<Expr>),               // -a or !a
    Func(String, Vec<Expr>),                   // avg(cpu, 5s)
    TimeWindow(String, u64),                   // cpu, 5000ms
}