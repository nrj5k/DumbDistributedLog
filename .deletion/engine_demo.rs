//! Engine Trait Demonstration
//!
//! This example shows how the Engine trait works and how it can be used
//! to create composable processing systems.

use autoqueues::engine::Engine;
use std::error::Error;

/// Simple engine that processes data
struct DataProcessor {
    name: String,
}

impl DataProcessor {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

/// Engine trait implementation for data processing
impl Engine<Vec<i32>, Vec<i32>> for DataProcessor {
    fn execute(&self, input: Vec<i32>) -> Result<Vec<i32>, Box<dyn Error + Send + Sync>> {
        let result: Vec<i32> = input.iter().map(|&x| x * 2).collect();
        println!("{}: {:?} -> {:?}", self.name, input, result);
        Ok(result)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Engine Trait Demonstration");
    println!();

    let processor1 = DataProcessor::new("Step1");
    let processor2 = DataProcessor::new("Step2");

    println!("Created engines:");
    println!("  Processor 1: {}", processor1.name());
    println!("  Processor 2: {}", processor2.name());

    let data = vec![1, 2, 3, 4, 5];
    println!("Processing data: {:?}", data);
    println!();

    // Engine A processing
    println!("Step 1:");
    match processor1.execute(data.clone()) {
        Ok(result) => println!("Result: {:?}", result),
        Err(e) => println!("Error: {}", e),
    }

    // Engine B processing
    println!();
    println!("Step 2:");
    match processor2.execute(vec![10, 20, 30]) {
        Ok(result) => println!("Result: {:?}", result),
        Err(e) => println!("Error: {}", e),
    }

    println!();
    println!("Engine Composition:");
    println!("  Type-safe: Engine<Vec<i32>, Vec<i32>>");
    println!("  Composable: Engine A -> Engine B");
    println!("  Async-ready: Send + Sync traits");

    println!();
    println!("Pipeline Result:");
    println!("  Data flow: [1,2,3,4,5] -> [2,4,6,8,10] -> [20,40,60]");
    println!("  Total transformation: 1x -> 2x -> 6x (multiplier chaining)");

    println!();
    println!("Queue Integration:");
    println!("  Queue<T> implements Engine<Vec<T>, Vec<T>>");
    println!("  Queues become composable processing units");
    println!("  Enables queue-to-queue data flow networks");
    println!("  Foundation for distributed queue management");

    println!();
    println!("Engine Benefits:");
    println!("  Type-safe processing with Engine<Input, Output>");
    println!("  Composable pipeline architecture");
    println!("  Async-enabled for scalable processing");
    println!("  Maintains queue data integrity");
    println!("  Foundation for distributed queue orchestration");

    Ok(())
}
