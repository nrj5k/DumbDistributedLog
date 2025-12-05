//! Simple Engine Usage Example
//!
//! Demonstrates the ultra-minimal Engine<Input, Output> trait with three
//! practical usage patterns: auto, manual, and functional approaches.

use autoqueues::engine::Engine;

// Simple health data structure
#[derive(Debug, Clone)]
struct HealthInput {
    cpu: f32,
    memory: f32,
    disk: f32,
}

#[derive(Debug, Clone)]
struct HealthOutput {
    score: f32,
    status: &'static str,
}

// 1) Auto way - minimal setup
struct AutoEngine;

impl Engine<HealthInput, HealthOutput> for AutoEngine {
    fn execute(
        &self,
        input: HealthInput,
    ) -> Result<HealthOutput, Box<dyn std::error::Error + Send + Sync>> {
        let score = (input.cpu + input.memory + input.disk) / 3.0;
        Ok(HealthOutput {
            score,
            status: if score >= 70.0 {
                "HEALTHY"
            } else {
                "NEEDS_ATTENTION"
            },
        })
    }

    fn name(&self) -> &str {
        "AutoEngine"
    }
}

// 2) Manual way - simple control
struct ManualEngine;

impl Engine<HealthInput, HealthOutput> for ManualEngine {
    fn execute(
        &self,
        input: HealthInput,
    ) -> Result<HealthOutput, Box<dyn std::error::Error + Send + Sync>> {
        let score = input.cpu * 0.5 + input.memory * 0.3 + input.disk * 0.2;
        let status = match score {
            s if s >= 80.0 => "EXCELLENT",
            s if s >= 60.0 => "GOOD",
            _ => "CAUTIOUS",
        };

        Ok(HealthOutput { score, status })
    }

    fn name(&self) -> &str {
        "ManualEngine"
    }
}

// 3) Functional way - closure-based
struct FunctionalEngine {
    compute_fn: Box<dyn Fn(&HealthInput) -> HealthOutput + Send + Sync>,
}

impl Engine<HealthInput, HealthOutput> for FunctionalEngine {
    fn execute(
        &self,
        input: HealthInput,
    ) -> Result<HealthOutput, Box<dyn std::error::Error + Send + Sync>> {
        Ok((self.compute_fn)(&input))
    }

    fn name(&self) -> &str {
        "FunctionalEngine"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Simple Engine Usage Example");
    println!("===========================");

    // Use simulated system metrics for the demo
    let health_input = HealthInput {
        cpu: 45.5,
        memory: 67.2,
        disk: 12.8,
    };

    println!(
        "System: CPU: {:.1}% Memory: {:.1}% Disk: {:.1}%",
        health_input.cpu, health_input.memory, health_input.disk
    );

    // Run engines and show results
    let engines = vec![
        Box::new(AutoEngine) as Box<dyn Engine<HealthInput, HealthOutput>>,
        Box::new(ManualEngine),
        Box::new(FunctionalEngine {
            compute_fn: Box::new(|input: &HealthInput| -> HealthOutput {
                let score = input.cpu.clamp(0.0, 100.0);
                HealthOutput {
                    score,
                    status: "OPTIMIZED",
                }
            }),
        }),
    ];

    println!("\n🔧 Engine Results:");
    for engine in engines {
        let result = engine.execute(health_input.clone())?;
        println!("Engine: {}", engine.name());
        println!("  Health Score: {:.1}", result.score);
        println!("  Status: {}", result.status);
    }

    println!("\n🎉 Engine trait successfully demonstrates:");
    println!("  • Type-safe I/O processing");
    println!("  • Multiple implementation patterns");
    println!("  • Zero abstraction overhead");
    println!("  • Clean functional approach");

    Ok(())
}
