//! Simple Expression Demo - KISS Implementation
//!
//! Demonstrates the new simplified expression engine with basic arithmetic
//! and local/global variable substitution.

use autoqueues::expression::{Expression, SimpleExpression};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧮 Simple Expression Engine Demo - KISS Implementation");
    println!("{}", "=".repeat(60));

    println!("\n🎯 Expression Examples:");
    println!("   • (local.cpu + global.memory) / 2.0");
    println!("   • local.capacity * 0.8");
    println!("   • (local.a + local.b) - global.c");

    // Test expression parsing
    println!("\n🔍 Testing Expression Parser:");

    let test_expressions = vec![
        "(local.cpu + global.memory) / 2.0",
        "local.capacity * 0.8",
        "(local.a + local.b) - global.c",
    ];

    for expr in test_expressions {
        match SimpleExpression::new(expr) {
            Ok(expression) => {
                println!("✅ Parsed: {}", expr);
                println!("   Local vars: {:?}", expression.required_local_vars());
                println!("   Global vars: {:?}", expression.required_global_vars());
            }
            Err(e) => {
                println!("❌ Failed to parse '{}': {}", expr, e);
            }
        }
    }

    // Test expression evaluation
    println!("\n🔄 Testing Expression Evaluation:");

    let expression = SimpleExpression::new("(local.cpu + global.memory) / 2.0")?;

    let mut local_vars = HashMap::new();
    local_vars.insert("cpu".to_string(), 35.0);
    local_vars.insert("memory".to_string(), 45.0);

    let mut global_vars = HashMap::new();
    global_vars.insert("memory".to_string(), 65.0);

    match expression.evaluate(&local_vars, &global_vars) {
        Ok(result) => {
            println!("✅ Evaluation result: {}", result);
            println!("   (35.0 + 65.0) / 2.0 = {}", result);
        }
        Err(e) => {
            println!("❌ Evaluation error: {}", e);
        }
    }

    // Test division by zero protection
    println!("\n🛡️  Testing Division by Zero Protection:");

    let zero_expr = SimpleExpression::new("local.value / 0.0")?;
    local_vars.clear();
    local_vars.insert("value".to_string(), 100.0);

    match zero_expr.evaluate(&local_vars, &global_vars) {
        Ok(_) => {
            println!("❌ Division by zero not caught!");
        }
        Err(e) => {
            println!("✅ Division by zero caught: {}", e);
        }
    }

    // Test undefined variable error
    println!("\n❓ Testing Undefined Variable Error:");

    let undefined_expr = SimpleExpression::new("local.missing + global.memory")?;
    local_vars.clear();
    // Don't add "missing" variable

    match undefined_expr.evaluate(&local_vars, &global_vars) {
        Ok(_) => {
            println!("❌ Undefined variable not caught!");
        }
        Err(e) => {
            println!("✅ Undefined variable caught: {}", e);
        }
    }

    println!("\n📋 KISS Expression Engine Features:");
    println!("   ✅ Ultra-minimal Expression trait (3 methods)");
    println!("   ✅ Simple arithmetic: + - * /");
    println!("   ✅ Local and global variable support");
    println!("   ✅ Division by zero protection");
    println!("   ✅ Clear error messages");
    println!("   ✅ Easy to extend later");

    println!("\n🎯 Next Steps:");
    println!("   • Integrate with simplified queue system");
    println!("   • Add to unified configuration system");
    println!("   • Test with real system metrics");

    println!("\n✅ Simple expression engine demo complete!");
    println!("   Ready for KISS queue integration!");

    Ok(())
}
