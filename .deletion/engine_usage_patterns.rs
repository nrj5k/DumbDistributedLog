//! Engine Usage Patterns - KISS Style
//!
//! Shows the AutoQueues Engine<Input, Output> trait in 3 smart usage patterns:
//! 1. Auto way - minimal setup
//! 2. Manual way - simple control  
//! 3. Functional way - closure-based

use autoqueues::*;

// 🏥 Health data structure (clean, simple)
#[derive(Debug, Clone)]
struct HealthInput {
    cpu: f32,
    memory: f32,
    disk: f32,
}

// Engine output - straightforward
#[derive(Debug, Clone)]
struct HealthData {
    score: f32,
    status: &'static str,
    message: String,
}

//////////////////////// 1️⃣ AUTO WAY ////////////////////////
// "Just give me health" - minimal setup

struct AutoHealthEngine;

impl Engine<HealthInput, HealthData> for AutoHealthEngine {
    fn execute(&self, input: HealthInput) -> Result<HealthData, Box<dyn std::error::Error + Send + Sync>> {
        let score = (input.cpu + input.memory + input.disk) / 3.0;
        Ok(HealthData {
            score,
            status: Self::get_status(score),
            message: format!("Auto: {:.1}/100", score),
        })
    }
    
    fn name(&self) -> &str { "AutoHealthEngine" }
}

impl AutoHealthEngine {
    fn get_status(score: f32) -> &'static str {
        match score {
            s if s >= 80.0 => "EXCELLENT",
            s if s >= 60.0 => "GOOD", 
            s if s >= 30.0 => "OK",
            _ => "NEEDS ATTENTION",
        }
    }
}

//////////////////////// 2️⃣ MANUAL WAY ////////////////////////
// "Let me control some details"  

struct ManualHealthEngine;

impl Engine<HealthInput, HealthData> for ManualHealthEngine {
    fn execute(&self, input: HealthInput) -> Result<HealthData, Box<dyn std::error::Error + Send + Sync>> {
        // Simple weighted calculation with penalties
        let mut score = 0.0;
        score += if input.cpu > 70.0 { input.cpu * 0.8 } else { input.cpu };
        score += if input.memory > 80.0 { input.memory * 0.7 } else { input.memory };
        score += if input.disk > 90.0 { input.disk * 0.5 } else { input.disk };
        
        let score = score / 3.0;
        
        Ok(HealthData {
            score,
            status: Self::get_status(score, &input),
            message: format!("Manual: {:.1}/100", score),
        })
    }
    
    fn name(&self) -> &str { "ManualHealthEngine" }
}

impl ManualHealthEngine {
    fn get_status(score: f32, input: &HealthInput) -> &'static str {
        if input.cpu > 80.0 { "SLOW_DOWN" }
        else if score >= 70.0 { "EXCELLENT" }
        else if score >= 50.0 { "GOOD" }
        else { "CAREFUL" }
    }
}

//////////////////////// 3️⃣ FUNCTIONAL WAY ////////////////////////
// "Just let me write the logic directly"

struct FunctionalHealthEngine<F> 
where F: Fn(&HealthInput) -> HealthData + Send + Sync
{
    compute_fn: F,
}

impl<F> Engine<HealthInput, HealthData> for FunctionalHealthEngine<F>
where F: Fn(&HealthInput) -> HealthData + Send + Sync
{
    fn execute(&self, input: HealthInput) -> Result<HealthData, Box<dyn std::error::Error + Send + Sync>> {
        Ok((self.compute_fn)(&input))
    }
    
    fn name(&self) -> &str { "FunctionalHealthEngine" }
}

//////////////////////// 🏃 MAIN DEMO ////////////////////////

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Engine Usage Patterns - KISS Demo");
    println!("{}", "=".repeat(55));
    
    // Get real system metrics
    let mut collector = MetricsCollector::new();
    collector.refresh();
    
    let cpu = collector.get_cpu_usage();
    let memory = collector.get_memory_usage().usage_percent;
    let disk = collector.get_disk_usage().map(|d| d.usage_percent).unwrap_or(0.0);
    
    let health_input = HealthInput { cpu, memory, disk };
    println!("📊 System: CPU:{:.1}% Memory:{:.1}% Disk:{:.1}%", cpu, memory, disk);
    
    // 1️⃣ AUTO way - "Just give me health"
    println!("\n🤖 AUTO way (simplest)");
    let auto_engine = AutoHealthEngine;
    let auto_result = auto_engine.execute(health_input.clone())?;
    println!("   Health: {} - {}", auto_result.status, auto_result.message);
    
    // 2️⃣ MANUAL way - "Let me control some details"
    println!("\n🛠️ MANUAL way (controlled)");
    let manual_engine = ManualHealthEngine;
    let manual_result = manual_engine.execute(health_input.clone())?;
    println!("   Health: {} - {}", manual_result.status, manual_result.message);
    
    // 3️⃣ FUNCTIONAL way - "Let me write the logic directly"
    println!("\n⚡ FUNCTIONAL way (direct)");
    let stress_engine = FunctionalHealthEngine::new(|input| {
        // Business logic right here
        let stress_index = input.cpu.max(input.memory.max(input.disk));
        let base_health = 100.0 - stress_index;
        
        HealthData {
            score: base_health.max(0.0),
            status: if stress_index > 70.0 { "BUSY" } else { "CHILL" },
            message: format!("Stress: {:.1}% [{}]", stress_index, 
                         if stress_index > 80.0 { "OVERLOADED" } else { "OK"}),
        }
    });
    
    let functional_result = stress_engine.execute(health_input)?;
    println!("   Health: {} - {}", functional_result.status, functional_result.message);
    
    // 🚀 Takeaways
    println!("\n✅ KISS Engine Demo Complete!");
    println!("   Engine<Input, Output> gives you 3 powerful usage patterns:");
    println!("   🎯 Auto   = Simple math    (health = average)");
    println!("   🛠️  Manual = Smart logic    (apply penalties)");
    println!("   ⚡ Code   = Total control   (write any function)");
    println!("\n   Pick your style. Engine trait stays the same.");
    println!("   Perfect for AutoQueues + TOML configuration!");
    
    Ok(())
}