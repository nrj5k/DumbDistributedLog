//! Simple Engine Trait Example - Fixed Version
//!
//! Demonstrates basic Engine<Input, Output> usage with proper error handling,
/// type conversions, and trait implementations.
use autoqueues::engine::Engine;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

// Custom error type for better diagnostics
#[derive(Debug)]
struct EngineError {
    message: String,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Engine Error: {}", self.message)
    }
}

impl Error for EngineError {}

// 1️⃣ Math Engine - Basic arithmetic expression evaluation
struct MathEngine;

#[derive(Debug, Clone)]
struct ExpressionInput {
    expression: String,
    variables: HashMap<String, f32>,
}

impl Engine<ExpressionInput, f32> for MathEngine {
    fn execute(&self, input: ExpressionInput) -> Result<f32, Box<dyn Error + Send + Sync>> {
        let result = match input.expression.as_str() {
            "sum" => input.variables.values().sum(),
            "average" => {
                let sum: f32 = input.variables.values().sum();
                sum / input.variables.len() as f32
            }
            "max" => {
                // Custom max for f32 (no Ord trait required)
                input
                    .variables
                    .values()
                    .fold(f32::MIN, |a, &b| if a > b { a } else { b })
            }
            "formula" => {
                // Health formula: (cpu * 0.4 + memory * 0.3 + disk * 0.3)
                input.variables.get("cpu").unwrap_or(&0.0) * 0.4
                    + input.variables.get("memory").unwrap_or(&0.0) * 0.3
                    + input.variables.get("disk").unwrap_or(&0.0) * 0.3
            }
            _ => {
                return Err(Box::new(EngineError {
                    message: "Unknown expression type".to_string(),
                }));
            }
        };

        Ok(result)
    }

    fn name(&self) -> &str {
        "MathEngine"
    }
}

// 2️⃣ String Engine - Text transformations
struct StringEngine;

impl Engine<String, String> for StringEngine {
    fn execute(&self, input: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        match input.as_str() {
            "health" => Ok("🟢 SYSTEM HEALTHY".to_string()),
            "status" => Ok("🔧 READY FOR OPERATION".to_string()),
            "alert" => Ok("⚠️ MONITORING REQUIRED".to_string()),
            _ => Ok(input.to_uppercase()),
        }
    }

    fn name(&self) -> &str {
        "StringEngine"
    }
}

// 3️⃣ Binary Engine - Data transformations
struct BinaryEngine;

impl Engine<Vec<u8>, Vec<u8>> for BinaryEngine {
    fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        // Simple transformation: reverse bytes
        let mut result = Vec::with_capacity(input.len());
        for byte in input.iter().rev() {
            result.push(*byte);
        }
        Ok(result)
    }

    fn name(&self) -> &str {
        "BinaryEngine"
    }
}

// Engine composition utilities
fn demo_engine_composition() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("\n4️⃣ Engine Composition - Multi-Type Flexibility");

    // Create different engines with their specific I/O types
    let math_engine = MathEngine;
    let string_engine = StringEngine;
    let binary_engine = BinaryEngine;

    println!("🎯 Different engines work with different I/O types:");
    println!("   Math:     ExpressionInput -> f32");
    println!("   String:   String -> String");
    println!("   Binary:   Vec<u8> -> Vec<u8>");

    // Demonstrate each engine individually
    let math_result = math_engine.execute(ExpressionInput {
        expression: "formula".to_string(),
        variables: HashMap::from([
            ("cpu".to_string(), 45.0),
            ("memory".to_string(), 60.0),
            ("disk".to_string(), 75.0),
        ]),
    })?;
    println!("   Health Score: {:.1}", math_result);

    let string_result = string_engine.execute("health".to_string())?;
    println!("   Status: {}", string_result);

    let binary_input = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    let binary_result = binary_engine.execute(binary_input)?;
    println!("   Binary: {:?}", binary_result);

    Ok(())
}

// Main function to run all demonstrations
fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("🚀 Engine Trait Demo Showcasing Type-Safe Processing");
    println!("{}\n", "=".repeat(50));

    // Run the engine demonstration
    demo_engine_composition()?;

    println!("\n✅ All Engine demonstrations completed successfully!");
    println!("\nKey Engine Trait Benefits:");
    println!("• Type-safe generic I/O processing");
    println!("• Zero abstraction overhead");
    println!("• Maximum flexibility for domain-specific engines");
    println!("• Easy engine composition and testing");

    Ok(())
}
