//! Expression parser
//!
//! Parses string expressions into AST nodes.

use crate::expression::types::{BinaryOp, Expr, Literal, UnaryOp};
use std::collections::VecDeque;

/// Error type for parsing errors
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Invalid number format: {0}")]
    InvalidNumber(String),
    #[error("Unknown function: {0}")]
    UnknownFunction(String),
    #[error("Invalid syntax")]
    InvalidSyntax,
}

/// Expression parser
pub struct Parser {
    tokens: VecDeque<Token>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Identifier(String),
    Plus,
    Minus,
    Asterisk,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Equals,
    NotEquals,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
    Not,
    Dot,
    Eof,
}

impl Parser {
    /// Parse an expression string into an AST
    pub fn parse(input: &str) -> Result<Expr, ParseError> {
        let tokens = Self::tokenize(input)?;
        let mut parser = Parser { tokens };
        parser.parse_expression()
    }

    /// Tokenize the input string
    fn tokenize(input: &str) -> Result<VecDeque<Token>, ParseError> {
        let mut tokens = VecDeque::new();
        let mut chars = input.chars().peekable();
        let mut pos = 0;

        while let Some(&ch) = chars.peek() {
            match ch {
                ' ' | '\t' | '\n' | '\r' => {
                    chars.next();
                    pos += 1;
                }
                '0'..='9' => {
                    let start = pos;
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() || ch == '.' {
                            chars.next();
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    let num_str = &input[start..pos];
                    let num = num_str
                        .parse::<f64>()
                        .map_err(|_| ParseError::InvalidNumber(num_str.to_string()))?;
                    tokens.push_back(Token::Number(num));
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let start = pos;
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                            chars.next();
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    let ident = &input[start..pos];
                    let token = match ident {
                        "true" => Token::Number(1.0),
                        "false" => Token::Number(0.0),
                        "and" => Token::And,
                        "or" => Token::Or,
                        "not" => Token::Not,
                        _ => Token::Identifier(ident.to_string()),
                    };
                    tokens.push_back(token);
                }
                '+' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Plus);
                }
                '-' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Minus);
                }
                '*' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Asterisk);
                }
                '/' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Slash);
                }
                '^' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Caret);
                }
                '(' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::LParen);
                }
                ')' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::RParen);
                }
                ',' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Comma);
                }
                '!' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'=') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::NotEquals);
                    } else {
                        tokens.push_back(Token::Not);
                    }
                }
                '=' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'=') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::Equals);
                    } else {
                        return Err(ParseError::UnexpectedChar('='));
                    }
                }
                '&' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'&') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::And);
                    } else {
                        return Err(ParseError::UnexpectedChar('&'));
                    }
                }
                '|' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'|') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::Or);
                    } else {
                        return Err(ParseError::UnexpectedChar('|'));
                    }
                }
                '>' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'=') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::GreaterEqual);
                    } else {
                        tokens.push_back(Token::Greater);
                    }
                }
                '<' => {
                    chars.next();
                    pos += 1;
                    if let Some(&'=') = chars.peek() {
                        chars.next();
                        pos += 1;
                        tokens.push_back(Token::LessEqual);
                    } else {
                        tokens.push_back(Token::Less);
                    }
                }
                '.' => {
                    chars.next();
                    pos += 1;
                    tokens.push_back(Token::Dot);
                }
                _ => {
                    return Err(ParseError::UnexpectedChar(ch));
                }
            }
        }

        tokens.push_back(Token::Eof);
        Ok(tokens)
    }

    /// Parse an expression (handles logical OR)
    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_logical_and()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::Or => {
                    self.tokens.pop_front();
                    let right = self.parse_logical_and()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse logical AND
    fn parse_logical_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_equality()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::And => {
                    self.tokens.pop_front();
                    let right = self.parse_equality()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::And, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse equality comparisons
    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::Equals => {
                    self.tokens.pop_front();
                    let right = self.parse_comparison()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Eq, Box::new(right));
                }
                Token::NotEquals => {
                    self.tokens.pop_front();
                    let right = self.parse_comparison()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Ne, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse comparison operators
    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::Greater => {
                    self.tokens.pop_front();
                    let right = self.parse_term()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Gt, Box::new(right));
                }
                Token::GreaterEqual => {
                    self.tokens.pop_front();
                    let right = self.parse_term()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Gte, Box::new(right));
                }
                Token::Less => {
                    self.tokens.pop_front();
                    let right = self.parse_term()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Lt, Box::new(right));
                }
                Token::LessEqual => {
                    self.tokens.pop_front();
                    let right = self.parse_term()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Lte, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse addition and subtraction
    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_factor()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::Plus => {
                    self.tokens.pop_front();
                    let right = self.parse_factor()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Add, Box::new(right));
                }
                Token::Minus => {
                    self.tokens.pop_front();
                    let right = self.parse_factor()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Sub, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse multiplication, division, and power
    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some(token) = self.tokens.front() {
            match token {
                Token::Asterisk => {
                    self.tokens.pop_front();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Mul, Box::new(right));
                }
                Token::Slash => {
                    self.tokens.pop_front();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Div, Box::new(right));
                }
                Token::Caret => {
                    self.tokens.pop_front();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(Box::new(left), BinaryOp::Pow, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse unary operators
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if let Some(token) = self.tokens.front() {
            match token {
                Token::Minus => {
                    self.tokens.pop_front();
                    let expr = self.parse_unary()?;
                    return Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr)));
                }
                Token::Not => {
                    self.tokens.pop_front();
                    let expr = self.parse_unary()?;
                    return Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)));
                }
                _ => {}
            }
        }

        self.parse_primary()
    }

    /// Parse primary expressions (literals, variables, function calls, parentheses)
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.tokens.pop_front() {
            Some(Token::Number(n)) => Ok(Expr::Literal(Literal::Float(n))),
            Some(Token::Identifier(name)) => {
                // Check if it's a function call
                if let Some(Token::LParen) = self.tokens.front() {
                    self.tokens.pop_front(); // consume '('
                    let mut args = Vec::new();

                    // Parse arguments
                    if let Some(Token::RParen) = self.tokens.front() {
                        // No arguments
                        self.tokens.pop_front(); // consume ')'
                    } else {
                        loop {
                            let arg = self.parse_expression()?;
                            args.push(arg);

                            if let Some(Token::RParen) = self.tokens.front() {
                                self.tokens.pop_front(); // consume ')'
                                break;
                            } else if let Some(Token::Comma) = self.tokens.front() {
                                self.tokens.pop_front(); // consume ','
                            } else {
                                return Err(ParseError::InvalidSyntax);
                            }
                        }
                    }

                    Ok(Expr::Func(name, args))
                } else if let Some(Token::Dot) = self.tokens.front() {
                    // Parse method call like "cpu.avg()"
                    self.tokens.pop_front(); // consume '.'
                    if let Some(Token::Identifier(method)) = self.tokens.pop_front() {
                        if let Some(Token::LParen) = self.tokens.front() {
                            self.tokens.pop_front(); // consume '('
                            
                            // Parse time window argument for methods like avg(5000)
                            let mut args = Vec::new();
                            if let Some(Token::Number(n)) = self.tokens.front().cloned() {
                                self.tokens.pop_front();
                                args.push(Expr::Literal(Literal::Float(n)));
                            }
                            
                            if let Some(Token::RParen) = self.tokens.front() {
                                self.tokens.pop_front(); // consume ')'
                            } else {
                                return Err(ParseError::InvalidSyntax);
                            }
                            
                            // For time window functions, create TimeWindow expr
                            if ["avg", "max", "min", "sum", "count"].contains(&method.as_str()) {
                                if let Some(Expr::Literal(Literal::Float(window_ms))) = args.get(0) {
                                    return Ok(Expr::TimeWindow(name, *window_ms as u64));
                                }
                            }
                            
                            // For other method calls, treat as function
                            let mut func_args = vec![Expr::Variable(name)];
                            func_args.extend(args);
                            Ok(Expr::Func(method, func_args))
                        } else {
                            Err(ParseError::InvalidSyntax)
                        }
                    } else {
                        Err(ParseError::InvalidSyntax)
                    }
                } else {
                    Ok(Expr::Variable(name))
                }
            }
            Some(Token::LParen) => {
                let expr = self.parse_expression()?;
                if let Some(Token::RParen) = self.tokens.pop_front() {
                    Ok(expr)
                } else {
                    Err(ParseError::InvalidSyntax)
                }
            }
            _ => Err(ParseError::UnexpectedEof),
        }
    }
}