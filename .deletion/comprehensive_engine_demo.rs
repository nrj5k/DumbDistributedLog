//! Comprehensive Engine Trait Example
//!
//! Smart demonstration of Engine<Input, Output> with complex architectures

use autoqueues::{engine::{Engine, GenericEngine}, MetricsCollector};
use std::sync::{Arc, Mutex};

// System Health Engine - Real system metrics analysis
struct HealthEngine {
    metrics: Arc<Mutex<MetricsCollector>>,
}

#[derive(Debug, Clone)]
struct HealthInput {
    health_formula: String,
    weights: HashMap<String, f32>,
}

impl Engine<HealthInput, f32> for HealthEngine {
    fn execute(&self, input: HealthInput) -> Result<f32, Box<dyn Error + Send + Sync>> {
        // Refresh real system metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.refresh();
        
        let cpu = metrics.get_cpu_usage();
        let memory = metrics.get_memory_usage().usage_percent;
        let disk = metrics.get_disk_usage()
            .ok_or("disk metrics unavailable")?
            .usage_percent;
        
        match input.health_formula.as_str() {
            "weighted_health" => {
                let cpu_weight = input.weights.get("cpu").unwrap_or(&0.4);
                let mem_weight = input.weights.get("memory").unwrap_or(&0.3);
                let disk_weight = input.weights.get("disk").unwrap_or(&0.3);
                
                Ok(cpu * cpu_weight + memory * mem_weight + disk * disk_weight)
            }
            "stress_index" => {
                // CPU weighted higher for stress calculation
                Ok(cpu * 0.6 + memory * 0.4)
            }
            "simple_average" => {
                Ok((cpu + memory + disk) / 3.0)
            }
            _ => Err("Unknown health formula".to_string().into())
        }
    }
    
    fn name(&self) -> &str {
        "HealthEngine"
    }
}

// Anomaly Detection Engine - Basic ML-style analysis
struct AnomalyEngine {
    baseline_stats: HashMap<String, f32>,
    threshold: f32,
}

#[derive(Debug, Clone)]
struct MetricInput {
    metric_name: String,
    current_value: f32,
    historical_values: Vec<f32>,
}

impl Engine<MetricInput, AnomalyResult> for AnomalyEngine {
    fn execute(&self, input: MetricInput) -> Result<AnomalyResult, Box<dyn Error + Send + Sync>> {
        let baseline = self.baseline_stats.get(&input.metric_name).ok_or("No baseline for metric")?;
        let deviation = ((input.current_value - baseline) / baseline).abs();
        
        // Statistical analysis
        let has_anomaly = deviation > self.threshold;
        let confidence = (deviation / self.threshold).min(1.0);
        let severity = match deviation {
            d if d > 0.5 => AnomalySeverity::Critical,
            d if d > 0.3 => AnomalySeverity::High,
            d if d > 0.2 => AnomalySeverity::Medium,
            _ => AnomalySeverity::Low,
        };
        
        // Trend analysis
        let trend = if input.historical_values.len() >= 2 {
            let recent = &input.historical_values[input.historical_values.len()-2..];
            if recent[1] > recent[0] { "increasing" } else { "decreasing" }.to_string()
        } else {
            "stable".to_string()
        };
        
        Ok(AnomalyResult {
            has_anomaly,
            confidence,
            severity,
            baseline: *baseline,
            current_value: input.current_value,
            trend,
        })
    }
    
    fn name(&self) -> &str {
        "AnomalyEngine"
    }
}

#[derive(Debug, Clone)]
struct AnomalyResult {
    has_anomaly: bool,
    confidence: f32,
    severity: AnomalySeverity,
    baseline: f32,
    current_value: f32,
    trend: String,
}

#[derive(Debug, Clone, PartialEq)]
enum AnomalySeverity {
    Low, Medium, High, Critical
}

// Complex Engine Composition Demo
struct EngineOrchestrator<I, O> {
    primary_engine: Box<dyn Engine<I, O>>,
    fallback_engine: Option<Box<dyn Engine<I, O>>>,
    validator: fn(&O) -> bool,
}

impl<I, O> EngineOrchestrator<I, O>
where
    I: Clone + Send + Sync,
    O: Clone + Send + Sync,
{
    fn new(
        primary: Box<dyn Engine<I, O>>,
        fallback: Option<Box<dyn Engine<I, O>>>,
        validator: fn(&O) -> bool,
    ) -> Self {
        Self {
            primary_engine: primary,
            fallback_engine: fallback,
            validator,
        }
    }
    
    fn execute_with_fallback(&self, input: I) -> Result<O, Box<dyn Error + Send + Sync>> {
        // Try primary engine
        match self.primary_engine.execute(input.clone()) {
            Ok(result) => {
                if (self.validator)(&result) {
                    Ok(result)
                } else if let Some(ref fallback) = self.fallback_engine {
                    println!("⚠️  Primary engine result failed validation, using fallback");
                    fallback.execute(input)
                } else {
                    Err("Primary engine result invalid and no fallback available".to_string().into())
                }
            }
            Err(e) => {
                if let Some(ref fallback) = self.fallback_engine {
                    println!("⚠️  Primary engine failed: {}, using fallback", e);
                    fallback.execute(input)
                } else {
                    Err(e)
                }
            }
        }
    }
}

fn main() {
    println!("🚀 Comprehensive Engine Trait Demo - Advanced Usage Patterns");
    println!("{}", "=".repeat(70));
    
    example_real_time_system().expect("Real-time system demo should work");
    example_anomaly_detection().expect("Anomaly detection demo should work"); 
    example_engine_orchestration().expect("Engine orchestration demo should work");
    
    println!("\n✅ Comprehensive Engine Demo Complete!");
    println!("\n🎯 Key Insights:");
    println!("   1. Engine<I,O> enables complex domain-specific processing");
    println!("   2. Type safety with generics I/O mapping");
    println!("   3. Error handling with detailed diagnostics");
    println!("   4. Engine composition patterns for reliability");
    println!("   5. Perfect for AutoQueues health + anomaly systems");
}

fn example_real_time_system() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("\n1️⃣ Real-Time System Health Monitoring");
    
    // 1. Health Engine
    let mut health_engine = HealthEngine {
        metrics: MetricsCollector::new(),
    };
    
    // Different health formulas with weights
    let health_inputs = vec![
        HealthInput {
            health_formula: "weighted_health".to_string(),
            weights: HashMap::from([
                ("cpu".to_string(), 0.4),
                ("memory".to_string(), 0.35),
                ("disk".to_string(), 0.25),
            ]),
        },
        HealthInput {
            health_formula: "stress_index".to_string(),
            weights: HashMap::new(), // Uses default weights
        },
    ];
    
    println!("🏥 Monitoring system health with different formulas:");
    for input in health_inputs {
        let health_result = health_engine.execute(input)?;
        let status = match health_result {
            h if h >= 80.0 => "🟢 EXCELLENT",
            h if h >= 60.0 => "🟡 GOOD", 
            h if h >= 30.0 => "🟠 WARNING",
            _ => "🔴 CRITICAL",
        };
        println!("   Health Score: {:>6.1}/100 {}", health_result, status);
    }
    
    // 2. Custom formula example
    let custom_health = HealthInput {
        health_formula: "simple_average".to_string(),
        weights: HashMap::new(),
    };
    
    let custom_result = health_engine.execute(custom_health)?;
    println!("   Average Load: {:>6.1}/100", custom_result);
    
    // Engine diagnostics showing real system data
    println!("   Real System Metrics:");
    health_engine.metrics.refresh();
    println!("   CPU Usage:    {:.1}%", health_engine.metrics.get_cpu_usage());
    println!("   Memory Usage: {:.1}%", health_engine.metrics.get_memory_usage().usage_percent);
    
    Ok(())
}

fn example_anomaly_detection() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("\n2️⃣ Anomaly Detection & Machine Learning Style Analysis");
    
    let anomaly_engine = AnomalyEngine {
        baseline_stats: HashMap::from([
            ("cpu_usage".to_string(), 45.0),
            ("memory_usage".to_string(), 60.0),
            ("disk_usage".to_string(), 75.0),
        ]),
        threshold: 0.25, // 25% deviation triggers anomaly
    };
    
    // Simulated system metrics with historical data
    let metrics = vec![
        MetricInput {
            metric_name: "cpu_usage".to_string(),
            current_value: 85.0, // 40% above baseline
            historical_values: vec![45.0, 47.0, 50.0, 85.0],
        },
        MetricInput {
            metric_name: "memory_usage".to_string(),
            current_value: 58.0, // -3% from baseline
            historical_values: vec![60.0, 61.0, 59.0, 58.0],
        },
    ];
    
    println!("🔍 Anomaly Detection Results:");
    for metric in metrics {
        let result = anomaly_engine.execute(metric)?;
        
        let severity_emojis = match result.severity {
            AnomalySeverity::Critical => "🔴🔴",
            AnomalySeverity::High => "🟠",
            AnomalySeverity::Medium => "🟡", 
            AnomalySeverity::Low => "⚪",
        };
        
        let result_text = if result.has_anomaly {
            format!("{severity_emojis} ANOMALY - Confidence {:.1}% - Trend: {}",
                   result.confidence * 100.0, result.trend)
        } else {
            format!("{severity_emojis} NORMAL - Confidence {:.1}%",
                   (1.0 - result.confidence) * 100.0)
        };
        
        println!("   Metric '{:?}': {}", result.current_value, result_text);
    }
    
    Ok(())
}

fn example_engine_orchestration() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("\n3️⃣ Engine Orchestration - Complex Reliability Patterns");
    
    // 1. Primary health engine
    let primary_health = HealthEngine {
        metrics: MetricsCollector::new(),
    };
    
    // 2. Health validation function
    let health_validator = |health_score: &f32| {
        *health_score >= 0.0 && *health_score <= 100.0
    };
    
    // 3. Orchestrator with health engine
    let health_orchestrator = EngineOrchestrator::<HealthInput, f32>::new(
        Box::new(primary_health),
        None, // No fallback for this demo
        health_validator,
    );
    
    // 4. Multiple orchestration scenarios
    println!("🔄 Engine Orchestration Results:");
    
    // Scenario 1: Normal operation
    let normal_input = HealthInput {
        health_formula: "weighted_health".to_string(),
        weights: HashMap::from([
            ("cpu".to_string(), 0.4),
            ("memory".to_string(), 0.3),
            ("disk".to_string(), 0.3),
        ]),
    };
    
    match health_orchestrator.execute_with_fallback(normal_input) {
        Ok(score) => println!("   Normal Health Score: {:.1}/100 ✅", score),
        Err(e) => println!("   Orchestration failed: {} ❌", e),
    }
    
    // Scenario 2: Stress scenario
    let stress_metrics = HashMap::from([
        ("cpu".to_string(), 0.7),    // Higher CPU weight
        ("memory".to_string(), 0.2),
        ("disk".to_string(), 0.1),
    ]);
    
    let stress_input = HealthInput {
        health_formula: "stress_index".to_string(),
        weights: stress_metrics,
    };
    
    match health_orchestrator.execute_with_fallback(stress_input) {
        Ok(score) => println!("   Stress Health Score: {:.1}/100 ⚠️", score),
        Err(e) => println!("   Edge case: {} ⚠️", e),
    }
    
    // 5. Performance timing
    let start = Instant::now();
    let _final_result = health_orchestrator.execute_with_fallback(HealthInput {
        health_formula: "weighted_health".to_string(),
        weights: HashMap::from([("cpu".to_string(), 0.4), ("memory".to_string(), 0.3), ("disk".to_string(), 0.3)]),
    })?;
    let elapsed = start.elapsed();
    
    println!("   Engine Execution: {:.2}ms for full health analysis", elapsed.as_micros() as f64 / 1000.0);
    println!("   Throughput: ~{:.0} health analyses per second", 1000.0 / elapsed.as_micros() as f64 * 1000.0);
    
    Ok(())
}