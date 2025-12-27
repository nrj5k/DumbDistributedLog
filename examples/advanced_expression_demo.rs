//! Advanced expression engine demo for AutoQueues
//!
//! Demonstrates enhanced mathematical functions and operations.

use autoqueues::{Expression, SimpleExpression};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧮 AutoQueues Advanced Expression Engine Demo");
    println!("===========================================");

    // Setup test variables
    let mut local_vars = HashMap::new();
    local_vars.insert("cpu".to_string(), 75.0);
    local_vars.insert("memory".to_string(), 60.0);
    local_vars.insert("disk".to_string(), 45.0);
    local_vars.insert("temperature".to_string(), -5.0);

    let mut global_vars = HashMap::new();
    global_vars.insert("network".to_string(), 30.0);
    global_vars.insert("power".to_string(), 85.0);

    // Demo 1: Basic arithmetic
    println!("\n1. Basic Arithmetic Operations:");
    println!("--------------------------------");

    // Test simple variable first
    let simple_var = SimpleExpression::<f64>::new("local.cpu")?;
    let result: f64 = simple_var.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {}", simple_var.expression, result);

    let addition = SimpleExpression::<f64>::new("local.cpu + local.memory")?;
    let result: f64 = addition.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {}", addition.expression, result);

    let multiplication = SimpleExpression::<f64>::new("local.cpu * local.disk")?;
    let result: f64 = multiplication.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {}", multiplication.expression, result);

    // Demo 2: Advanced math functions
    println!("\n2. Advanced Math Functions:");
    println!("--------------------------");

    let sqrt_expr = SimpleExpression::<f64>::new("sqrt(local.cpu)")?;
    let result: f64 = sqrt_expr.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {:.2}", sqrt_expr.expression, result);

    let abs_expr = SimpleExpression::<f64>::new("abs(local.temperature)")?;
    let result: f64 = abs_expr.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {:.1}", abs_expr.expression, result);

    let pow_expr = SimpleExpression::<f64>::new("pow(local.memory, 2.0)")?;
    let result: f64 = pow_expr.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {:.1}", pow_expr.expression, result);

    // Demo 3: Comparison functions
    println!("\n3. Comparison Functions:");
    println!("-----------------------");

    let max_expr = SimpleExpression::<f64>::new("max(local.cpu, global.power)")?;
    let result: f64 = max_expr.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {:.1}", max_expr.expression, result);

    let min_expr = SimpleExpression::<f64>::new("min(local.memory, local.disk)")?;
    let result: f64 = min_expr.evaluate(&local_vars, &global_vars)?;
    println!("   {} = {:.1}", min_expr.expression, result);

    // Demo 4: Complex expressions
    println!("\n4. Complex Expressions:");
    println!("----------------------");

    // Health score calculation
    let health_expr = SimpleExpression::<f64>::new("(local.cpu + local.memory) / 2.0")?;
    let result: f64 = health_expr.evaluate(&local_vars, &global_vars)?;
    println!(
        "   Health Score: {} = {:.1}",
        health_expr.expression, result
    );

    // Performance index
    let perf_expr = SimpleExpression::<f64>::new("sqrt(local.cpu) * abs(local.temperature)")?;
    let result: f64 = perf_expr.evaluate(&local_vars, &global_vars)?;
    println!(
        "   Performance Index: {} = {:.2}",
        perf_expr.expression, result
    );

    // Demo 5: Variable extraction
    println!("\n5. Variable Extraction:");
    println!("----------------------");

    let complex_expr =
        SimpleExpression::<f64>::new("max(local.cpu, global.network) + min(local.memory, local.disk)")?;
    let local_vars_required = complex_expr.required_local_vars();
    let global_vars_required = complex_expr.required_global_vars();

    println!("   Expression: {}", complex_expr.expression);
    println!("   Local variables needed: {:?}", local_vars_required);
    println!("   Global variables needed: {:?}", global_vars_required);

    // Demo 6: Error handling
    println!("\n6. Error Handling:");
    println!("------------------");

    // Division by zero protection
    let div_zero = SimpleExpression::<f64>::new("local.cpu / 0.0")?;
    match div_zero.evaluate(&local_vars, &global_vars) {
        Ok(_) => println!("   ❌ Division by zero not caught!"),
        Err(e) => println!("   ✅ Division by zero correctly caught: {}", e),
    }

    // Undefined variable
    let undefined = SimpleExpression::<f64>::new("local.missing_var")?;
    match undefined.evaluate(&local_vars, &global_vars) {
        Ok(_) => println!("   ❌ Undefined variable not caught!"),
        Err(e) => println!("   ✅ Undefined variable correctly caught: {}", e),
    }

    println!("\n🎉 Advanced expression engine demo completed!");
    println!("📝 Features demonstrated:");
    println!("   ✅ Basic arithmetic (+, -, *, /)");
    println!("   ✅ Math functions (sqrt, abs, powf)");
    println!("   ✅ Comparison functions (max, min)");
    println!("   ✅ Complex expressions");
    println!("   ✅ Variable extraction");
    println!("   ✅ Error handling");

    Ok(())
}