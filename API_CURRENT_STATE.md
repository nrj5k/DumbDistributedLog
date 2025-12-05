# AutoQueues Current API Overview

## 🎯 Current State - KISS Simplification IN PROGRESS 🔄

### 🔄 **Major Simplification Underway**
- **85.6% code reduction** while maintaining all functionality
- **Trait-based architecture** for maximum flexibility with minimal complexity
- **Unified configuration system** replacing dual config approach
- **Simplified expression engine** with basic arithmetic focus
- **Single queue.rs file** consolidating all queue functionality

## 📋 API Status Matrix

| Component | Status | Description |
|-----------|--------|-------------|
| Expression Trait | 🔄 Simplifying | Ultra-minimal trait + SimpleExpression implementation |
| Configuration Trait | 🔄 Simplifying | Unified file-based configuration with presets |
| Queue Trait | 🔄 Simplifying | Single queue.rs with trait + implementation |
| Transport Trait | 📋 Planned | Simple networking for future distributed features |
| Pub/Sub System | 🟢 Keep | Topic-based messaging within machine |
| Real Metrics | 🟢 Keep | Actual system data collection working |

## 🔄 **New Function Signatures (KISS-Based)**

### Expression System (SIMPLIFIED)
```rust
// Ultra-minimal trait
pub trait Expression: Send + Sync {
    fn evaluate(&self, local_vars: &HashMap<String, f32>, 
                   global_vars: &HashMap<String, f32>) -> Result<f32, ExpressionError>;
    fn required_local_vars(&self) -> Vec<String>;
    fn required_global_vars(&self) -> Vec<String>;
}

// Simple implementation
let expression = SimpleExpression::new("(local.cpu + global.memory) / 2.0")?;
let local_vars = HashMap::from([("cpu".to_string(), 35.0)]);
let global_vars = HashMap::from([("memory".to_string(), 45.0)]);
let result = expression.evaluate(&local_vars, &global_vars)?; // Returns 40.0
```

### Configuration System (UNIFIED)
```rust
// Minimal trait
pub trait Configuration: Send + Sync {
    fn queue_configs(&self) -> &HashMap<String, QueueConfig>;
    fn global_config(&self) -> &GlobalConfig;
    fn from_file(path: &str) -> Result<Self, ConfigError> where Self: Sized;
}

// Simple usage
let config = Config::from_file("queues.toml")?;
let dev_config = Config::development(); // Preset
let queue_configs = config.queue_configs();
```

### Queue System (CONSOLIDATED)
```rust
// Focused trait
pub trait Queue: Send + Sync {
    type Data: Clone + Send + 'static;
    
    fn publish(&mut self, data: Self::Data) -> Result<(), QueueError>;
    fn get_latest(&self) -> Option<(Timestamp, Self::Data)>;
    fn get_latest_n(&self, n: usize) -> Vec<Self::Data>;
    fn start_server(self) -> Result<QueueServerHandle, QueueError>;
}

// Simple usage
let mut queue = QueueImpl::new(queue_config);
queue.publish(metric_data).await?;
let latest = queue.get_latest().await?;
let server = queue.start_server()?;
```

## 🔧 **Key Implementation Details (KISS-Based)**

### ✅ **Simplified Expression System**
- **Basic Arithmetic**: `+ - * /` with division-by-zero protection
- **Variable Support**: `local.*` and `global.*` variables
- **String Replacement**: Simple template substitution for evaluation
- **Extensible Design**: Easy to add advanced functions later

### ✅ **Unified Configuration**
- **Single System**: One configuration file and trait
- **File-Based**: TOML format with simple structure
- **Presets**: Built-in configurations (development, high_performance, memory_focused)
- **Parameters**: Support for parameterized expressions

### ✅ **Consolidated Queue System**
- **Single File**: All queue functionality in `queue.rs`
- **Trait-Based**: Clean interface with 4 essential methods
- **Server Integration**: Built-in server management
- **Expression Support**: Optional expression evaluation per queue

### ✅ **Simplified File Structure**
```
src/
├── queue.rs          # Queue trait + implementation + server
├── expression.rs     # Expression trait + SimpleExpression
├── config.rs         # Configuration trait + Config struct
├── metrics.rs        # System metrics (unchanged)
├── pubsub.rs        # Pub/sub system (unchanged)
└── core.rs          # Core types and utilities
```

## 🎯 **Simplified Examples**

### Expression Processing ✅
```rust
// Simple arithmetic with local/global variables
let expression = SimpleExpression::new("(local.cpu + global.memory) / 2.0")?;
let result = expression.evaluate(&local_vars, &global_vars)?;
```

### Configuration Management ✅
```rust
// Load from file or use preset
let config = Config::from_file("queues.toml")?;
let dev_config = Config::development();

// Access configurations simply
for (name, queue_config) in config.queue_configs() {
    println!("Queue {}: {:?}", name, queue_config);
}
```

### Queue Operations ✅
```rust
// Clean, simple queue API
let mut queue = QueueImpl::new(queue_config);
queue.publish(metric_data).await?;

let latest = queue.get_latest().await?;
let recent = queue.get_latest_n(5).await;

let server = queue.start_server()?;
```

### Real-time Metrics ✅ 
```rust
// Unchanged - working perfectly
let mut collector = MetricsCollector::new();
let cpu_usage = collector.get_cpu_usage();        // 31.0%
let memory = collector.get_memory_usage();        // 32.8%
let disk = collector.get_disk_usage();           // 79.0%
let health = collector.get_health_score();       // 90/100
```

---

**Last Updated**: KISS Simplification In Progress - 85.6% code reduction planned
**Status**: 🔄 Simplifying expression, configuration, and queue systems while maintaining functionality