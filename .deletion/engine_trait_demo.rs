//! Example: Engine Trait Demonstration - Clean Version
//!
//! This demonstrates the Engine trait with proper error handling

use autoqueues::engine::Engine;
use std::error::Error;
use std::fmt;

/// Simple Engine with clear error handling
struct MultiplierEngine {
    name: String,
    multiplier: i32,
}

impl MultiplierEngine {
    fn new(name: &str, multiplier: i32) -> Self {
        Self {
            name: name.to_string(),
            multiplier,
        }
    }
}

/// Custom error for this example
#[derive(Debug)]
struct EngineError(String);

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EngineError: {}", self.0)
    }
}

impl Error for EngineError {}

impl Engine<Vec<i32>, Vec<i32>> for MultiplierEngine {
    fn execute(&self, input: Vec<i32>) -> Result<Vec<i32>, Box<dyn Error + Send + Sync>> {
        println!(
            "🚀 {} processing: {:?} * {}",
            self.name, input, self.multiplier
        );
        let result: Vec<i32> = input.iter().map(|&x| x * self.multiplier).collect();
        Ok(result)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[tokio::main]
async fn main() {
    println!("🚀 Engine Trait Demonstration - Clean Error Handling\n");
    println!("🚀 Engine Trait Demonstration - Clean Error Handling\n");

    // Create engines
    let engine1 = MultiplierEngine::new("Double", 2);
    let engine2 = MultiplierEngine::new("Triple", 3);

    println!("✅ Created engines:");
    println!("   📊 Engine 1: {} ({}x)", engine1.name, engine1.multiplier);
    println!("   🔍 Engine 2: {} ({}x)", engine2.name, engine2.multiplier);

    // Test data pipeline
    let data = vec![1, 2, 3, 4, 5];
    println!("\n🔄 Processing pipeline with data: {:?}", data);

    // Process with Engine 1
    println!("\n🧮 Step 1 - Double Engine:");
    let mut final_result = vec![2, 4, 6, 8, 10]; // Expected from manual calculation
    match engine1.execute(data.clone()) {
        Ok(step1_result) => {
            println!("   ✅ Result: {:?}", step1_result);

            // Process with Engine 2
            println!("\n🎯 Step 2 - Triple Engine:");
            match engine2.execute(step1_result) {
                Ok(step2_result) => {
                    println!("   ✅ Result: {:?}", step2_result);
                    final_result = step2_result;
                }
                Err(e) => println!("   ❌ Engine 2 error: {}", e),
            }
        }
        Err(e) => println!("   ❌ Engine 1 error: {}", e),
    }

    // Demonstrate engine composition
    println!("\n🔗 Engine Composition:");
    println!("   • Type-safe: Engine<Vec<i32>, Vec<i32>>");
    println!("   • Chainable: Engine A → Engine B");
    println!("   • Error-safe: Box<dyn Error> handling");
    println!("   • Async-ready: Send + Sync traits");

    println!("\n📊 Pipeline Result:");
    println!(
        "\n🔄 Data flow: {:?} → {:?} → {:?}",
        data,
        vec![2, 4, 6, 8, 10],
        final_result
    );
    println!("   📈 Total transformation: 1x → 2x → 6x (2×3 multiplication)");

    // Show queue integration concept
    println!("\n🎯 Queue Integration:");
    println!("   ✅ Queue<T> implements Engine<Vec<T>, Vec<T>>");
    println!("   ✅ Queues become composable Engine components");
    println!("   ✅ Processing contracts enforced at compile-time");
    println!("   ✅ Foundation for distributed queue management");

    println!("\n✨ Engine Benefits:");
    println!("   • Type-safe processing with Engine<Input, Output>");
    println!("   • Composable pipeline architecture");
    println!("   • Error handling with Box<dyn Error + Send + Sync>");
    println!("   • Zero-cost abstractions for performance");
    println!("   • Distributed system foundation");
}
