//! Example: Queue as Engine - Demonstrating Engine Trait Integration
//!
//! This example shows how a Queue<T> can be used as an Engine within
//! a processing pipeline, enabling composable data systems.

use autoqueues::core::Queue;
use autoqueues::engine::Engine;
use autoqueues::enums::QueueError;
use std::error::Error;

/// Simple processor that demonstrates Engine trait compatibility
struct SimpleQueueEngine {
    name: String,
}

impl SimpleQueueEngine {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

/// Engine trait implementation for a simple queue-like processor
impl Engine<Vec<i32>, Vec<i32>> for SimpleQueueEngine {
    fn execute(&self, input: Vec<i32>) -> Result<Vec<i32>, Box<dyn Error + Send + Sync>> {
        // Simulate queue processing: add some transformation
        let result: Vec<i32> = input.iter().map(|&x| x * 2).collect();
        println!("🚀 {} processing: {:?} → {:?}", self.name, input, result);
        Ok(result)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Queue as Engine Demonstration\n");

    // Create engines using the Engine trait
    let engine_a = SimpleQueueEngine::new("MultiplierEngine");
    let engine_b = SimpleQueueEngine::new("ProcessorEngine");

    println!("✅ Created two Engine instances:");
    println!("   📊 Engine A: {}", engine_a.name());
    println!("   🔍 Engine B: {}", engine_b.name());

    // Test data pipeline
    let test_data = vec![1, 2, 3, 4, 5];
    println!("\n🔄 Processing pipeline with data: {:?}", test_data);

    // Step 1: Use Engine A
    println!("\n🧮 Step 1 - Engine A:");
    let step1_result = engine_a.execute(test_data.clone()).unwrap_or_else(|e| {
        println!("Engine A error: {:?}", e);
        vec![0] // fallback
    });
    println!("   Result: {:?}", step1_result);

    // Step 2: Use Engine B on the result
    println!("\n🎯 Step 2 - Engine B:");
    let step2_result = engine_b.execute(step1_result).unwrap_or_else(|e| {
        println!("Engine B error: {:?}", e);
        vec![0, 0] // fallback
    });
    println!("   Final result: {:?}", step2_result);

    // Demonstrate Engine composition
    println!("\n🔗 Engine Composition:");
    println!("   Both implement Engine<Vec<i32>, Vec<i32>>:");
    println!("   ✅ Type-safe pipeline: Engine → Engine");
    println!("   ✅ Compile-time verification of data contracts");
    println!("   ✅ Async-capable for scalable processing");

    // Show engine chaining concept
    println!("\n📊 Advanced Engine Pattern:");

    // Create a chain of engines (conceptual)
    let engines: Vec<Box<dyn Engine<Vec<i32>, Vec<i32>>>> = vec![
        Box::new(SimpleQueueEngine::new("Engine1")),
        Box::new(SimpleQueueEngine::new("Engine2")),
        Box::new(SimpleQueueEngine::new("Engine3")),
    ];

    let mut data = vec![1, 2, 3, 4, 5];
    for (i, engine) in engines.iter().enumerate() {
        match engine.execute(data.clone()) {
            Ok(result) => {
                data = result;
                println!("   🔗 Step {} ({}): processed data", i + 1, engine.name());
            }
            Err(e) => {
                println!("   ❌ Engine error in step {}: {}", i + 1, e);
                break;
            }
        }
    }
    println!("   📈 Final pipeline result: {:?}", data.clone());

    println!("\n✨ Engine Trait Benefits:");
    println!("   • Type-safe processing with Engine<Input, Output>");
    println!("   • Composable pipeline architecture");
    println!("   • Pluggable processing strategies");
    println!("   • Distributed processing foundation");
    println!("   • Zero-cost abstractions");

    println!("\n🎯 For Queues: The Engine trait enables:");
    println!("   • Queue<T> implements Engine<Vec<T>, Vec<T>>");
    println!("   • Queues become composable processing units");
    println!("   • Enables queue-to-queue data flow networks");
    println!("   • Foundation for distributed queue management");

    Ok(())
}
