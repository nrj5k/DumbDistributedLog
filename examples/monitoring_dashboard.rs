//! Real-time monitoring dashboard for AutoQueues
//!
//! Provides a simple console-based dashboard for monitoring system metrics,
//! queue performance, and expression evaluations.

use autoqueues::{Expression, MetricsCollector, SimpleExpression};
use std::collections::HashMap;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

struct Dashboard {
    metrics: MetricsCollector,
    expressions: Vec<SimpleExpression<f64>>,
    local_vars: HashMap<String, f64>,
    global_vars: HashMap<String, f64>,
}

impl Dashboard {
    fn new() -> Self {
        let mut dashboard = Self {
            metrics: MetricsCollector::new(),
            expressions: Vec::new(),
            local_vars: HashMap::new(),
            global_vars: HashMap::new(),
        };

        // Initialize with default expressions
        dashboard.setup_default_expressions();
        dashboard
    }

    fn setup_default_expressions(&mut self) {
        // System health expressions
        self.expressions
            .push(SimpleExpression::<f64>::new("local.cpu_percent").unwrap());
        self.expressions
            .push(SimpleExpression::<f64>::new("local.memory_percent").unwrap());
        self.expressions.push(
            SimpleExpression::<f64>::new("(local.cpu_percent + local.memory_percent) / 2.0").unwrap(),
        );
        self.expressions
            .push(SimpleExpression::<f64>::new("sqrt(local.cpu_percent) * 10.0").unwrap());
        self.expressions
            .push(SimpleExpression::<f64>::new("max(local.cpu_percent, local.memory_percent)").unwrap());
    }

    fn update_metrics(&mut self) {
        // Collect system metrics
        let system_metrics = self.metrics.get_all_metrics();

        // Update local variables
        self.local_vars.insert(
            "cpu_percent".to_string(),
            system_metrics.cpu_usage_percent as f64,
        );
        self.local_vars.insert(
            "memory_percent".to_string(),
            system_metrics.memory_usage.usage_percent as f64,
        );
        self.local_vars.insert(
            "disk_usage".to_string(),
            system_metrics.disk_usage.usage_percent as f64,
        );

        // Simulate some global variables
        self.global_vars.insert("network_latency".to_string(), 25.5);
        self.global_vars.insert("error_rate".to_string(), 0.02);
        self.global_vars.insert("throughput".to_string(), 850.0);
    }

    fn render(&self) {
        // Clear screen (works on most terminals)
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        println!("🔧 AutoQueues Real-Time Monitoring Dashboard");
        println!("{}", "=".repeat(50));
        println!("📅 Last Update: {}", chrono::Utc::now().format("%H:%M:%S"));
        println!();

        // System Metrics Section
        println!("📊 System Metrics");
        println!("{}", "─".repeat(20));
        println!(
            "   CPU Usage:     {:6.1}%",
            self.local_vars.get("cpu_percent").unwrap_or(&0.0)
        );
        println!(
            "   Memory Usage:  {:6.1}%",
            self.local_vars.get("memory_percent").unwrap_or(&0.0)
        );
        println!(
            "   Disk Usage:    {:6.1}%",
            self.local_vars.get("disk_usage").unwrap_or(&0.0)
        );
        println!();

        // Global Metrics Section
        println!("🌐 Global Metrics");
        println!("{}", "─".repeat(20));
        println!(
            "   Network Latency: {:6.1} ms",
            self.global_vars.get("network_latency").unwrap_or(&0.0)
        );
        println!(
            "   Error Rate:      {:6.3}%",
            self.global_vars.get("error_rate").unwrap_or(&0.0) * 100.0
        );
        println!(
            "   Throughput:      {:6.0} ops/s",
            self.global_vars.get("throughput").unwrap_or(&0.0)
        );
        println!();

        // Expression Evaluations Section
        println!("🧮 Expression Evaluations");
        println!("{}", "─".repeat(30));
        for (i, expr) in self.expressions.iter().enumerate() {
            match expr.evaluate(&self.local_vars, &self.global_vars) {
                Ok(value) => {
                    // Add color coding based on value
                    let status = if value > 80.0 {
                        "🔴 HIGH"
                    } else if value > 50.0 {
                        "🟡 MEDIUM"
                    } else {
                        "🟢 NORMAL"
                    };
                    println!(
                        "   {:2}. {:25} = {:8.2} {}",
                        i + 1,
                        expr.expression,
                        value,
                        status
                    );
                }
                Err(e) => {
                    println!("   {:2}. {:25} = ERROR: {}", i + 1, expr.expression, e);
                }
            }
        }
        println!();

        // Health Summary
        println!("💚 System Health Summary");
        println!("{}", "─".repeat(25));
        let cpu = self.local_vars.get("cpu_percent").unwrap_or(&0.0);
        let memory = self.local_vars.get("memory_percent").unwrap_or(&0.0);
        let health_score = (cpu + memory) / 2.0;

        let health_status = if health_score > 80.0 {
            "⚠️  HIGH LOAD"
        } else if health_score > 60.0 {
            "✅ MODERATE"
        } else {
            "🟢 HEALTHY"
        };

        println!(
            "   Overall Health: {:.1}% - {}",
            health_score, health_status
        );
        println!("   Active Expressions: {}", self.expressions.len());
        println!("   Update Interval: 1 second");
        println!();

        println!("Press Ctrl+C to exit...");
    }

    fn run(&mut self) {
        println!("🚀 Starting AutoQueues Monitoring Dashboard...");
        thread::sleep(Duration::from_millis(1000));

        loop {
            self.update_metrics();
            self.render();
            thread::sleep(Duration::from_millis(1000));
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if chrono is available, otherwise use simplified timestamp
    let mut dashboard = Dashboard::new();

    // Set up terminal for monitoring
    println!("🔧 AutoQueues Real-Time Monitoring Dashboard");
    println!("==========================================");
    println!("📝 Features:");
    println!("   ✅ Real-time system metrics (CPU, Memory, Disk)");
    println!("   ✅ Live expression evaluation");
    println!("   ✅ Health status monitoring");
    println!("   ✅ Color-coded status indicators");
    println!("   ✅ 1-second update intervals");
    println!();
    println!("🎯 Starting dashboard in 3 seconds...");
    println!("   (Press Ctrl+C to exit)");

    for i in (1..=3).rev() {
        print!("   {}...", i);
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_millis(1000));
    }

    println!("\n🎉 Dashboard started!");

    // Run the dashboard
    dashboard.run();

    Ok(())
}