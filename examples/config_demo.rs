//! Configuration Demo - KISS Unified Configuration System
//!
//! Demonstrates the simplified configuration system with TOML support.

use autoqueues::config::QueueConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Configuration Demo - KISS Unified System");
    println!("==========================================\n");

    // Test 1: Standard configuration
    println!("📋 Testing Standard Configuration:");
    let standard_config = QueueConfig::create_standard();

    println!("   Base metrics: {:?}", standard_config.get_base_metrics());
    println!("   Derived formulas:");
    for (name, formula) in standard_config.get_derived_formulas() {
        println!("     • {}: {}", name, formula);
    }
    println!();

    // Test 2: Minimal configuration
    println!("📋 Testing Minimal Configuration:");
    let minimal_config = QueueConfig::create_minimal();

    println!("   Base metrics: {:?}", minimal_config.get_base_metrics());
    println!("   Derived formulas:");
    for (name, formula) in minimal_config.get_derived_formulas() {
        println!("     • {}: {}", name, formula);
    }
    println!();

    // Test 3: Custom TOML configuration
    println!("📋 Testing Custom TOML Configuration:");
    let custom_toml = r#"
[queues]
base = ["cpu_usage", "memory_usage", "disk_io"]

[queues.derived.server_health]
formula = "(local.cpu_usage * 0.5) + (local.memory_usage * 0.3) + (local.disk_io * 0.2)"

[queues.derived.performance_index]
formula = "100.0 - ((local.cpu_usage + local.memory_usage + local.disk_io) / 3.0)"
"#;

    let custom_config = QueueConfig::from_str(custom_toml)?;
    println!("   Base metrics: {:?}", custom_config.get_base_metrics());
    println!("   Derived formulas:");
    for (name, formula) in custom_config.get_derived_formulas() {
        println!("     • {}: {}", name, formula);
    }
    println!();

    // Test 4: Error handling
    println!("🛡️ Testing Error Handling:");
    let invalid_toml = r#"
[queues]
base = ["cpu_percent"
"#; // Missing closing bracket

    match QueueConfig::from_str(invalid_toml) {
        Ok(_) => println!("   ❌ Should have failed!"),
        Err(e) => println!("   ✅ Caught error: {}", e),
    }
    println!();

    println!("🎯 KISS Configuration Features:");
    println!("   ✅ Single unified configuration file");
    println!("   ✅ TOML format for human readability");
    println!("   ✅ Base metrics + derived formulas");
    println!("   ✅ Expression-based health calculations");
    println!("   ✅ Built-in standard configurations");
    println!("   ✅ Clear error messages");
    println!("   ✅ Easy to extend and customize");

    println!("\n✅ Configuration demo complete!");
    println!("   Ready for queue integration!");

    Ok(())
}
