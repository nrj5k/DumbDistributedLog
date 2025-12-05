//! System Metrics Collection Module
//!
//! Provides real-time system information including CPU usage, memory consumption,
//! disk usage, and system information. Uses sysinfo crate for cross-platform
//! compatibility.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};

/// System metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: u64,
    pub cpu_usage_percent: f32,
    pub memory_usage: MemoryMetrics,
    pub disk_usage: DiskMetrics,
    pub system_info: SystemInfo,
}

/// Memory usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_gb: f32,
    pub used_gb: f32,
    pub free_gb: f32,
    pub usage_percent: f32,
    pub available_gb: f32,
}

/// Disk usage metrics for a specific disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub total_gb: f32,
    pub used_gb: f32,
    pub free_gb: f32,
    pub usage_percent: f32,
    pub disk_name: String,
    pub mount_point: String,
}

/// Basic system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub system_name: String,
    pub kernel_version: String,
    pub os_version: String,
    pub uptime_seconds: u64,
    pub process_count: usize,
}

/// System metrics collector
pub struct MetricsCollector {
    system: System,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self { system }
    }

    /// Refresh all system information
    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }

    /// Get current timestamp in milliseconds
    fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Calculate CPU usage percentage across all cores
    pub fn get_cpu_usage(&mut self) -> f32 {
        self.system.refresh_cpu_all();
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.system.refresh_cpu_all();

        self.system.global_cpu_usage()
    }

    /// Get memory usage metrics
    pub fn get_memory_usage(&mut self) -> MemoryMetrics {
        self.system.refresh_memory();

        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let available_memory = self.system.available_memory();
        let free_memory = self.system.free_memory();

        let bytes_to_gb = |bytes: u64| -> f32 { bytes as f32 / (1024.0 * 1024.0 * 1024.0) };

        MemoryMetrics {
            total_gb: bytes_to_gb(total_memory),
            used_gb: bytes_to_gb(used_memory),
            free_gb: bytes_to_gb(free_memory),
            usage_percent: (used_memory as f32 / total_memory as f32) * 100.0,
            available_gb: bytes_to_gb(available_memory),
        }
    }

    /// Get disk usage metrics for the root filesystem
    pub fn get_disk_usage(&self) -> Option<DiskMetrics> {
        let disks = Disks::new_with_refreshed_list();

        // Find root filesystem
        if let Some(disk) = disks
            .iter()
            .find(|disk| disk.mount_point().to_str().unwrap_or("") == "/")
        {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);

            let bytes_to_gb = |bytes: u64| -> f32 { bytes as f32 / (1024.0 * 1024.0 * 1024.0) };

            Some(DiskMetrics {
                total_gb: bytes_to_gb(total),
                used_gb: bytes_to_gb(used),
                free_gb: bytes_to_gb(available),
                usage_percent: (used as f32 / total as f32) * 100.0,
                disk_name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_str().unwrap_or("unknown").to_string(),
            })
        } else {
            None // No root filesystem found
        }
    }

    /// Get basic system information
    pub fn get_system_info(&mut self) -> SystemInfo {
        use sysinfo::ProcessesToUpdate;

        self.system.refresh_all();
        self.system.refresh_processes(ProcessesToUpdate::All, true);

        SystemInfo {
            system_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
            kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
            os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            uptime_seconds: System::uptime(),
            process_count: self.system.processes().len(),
        }
    }

    /// Get comprehensive system metrics
    pub fn get_all_metrics(&mut self) -> SystemMetrics {
        let timestamp = Self::get_timestamp();
        let cpu_usage = self.get_cpu_usage();
        let memory = self.get_memory_usage();
        let disk = self.get_disk_usage().unwrap_or_else(|| {
            // Fallback disk metrics
            DiskMetrics {
                total_gb: 0.0,
                used_gb: 0.0,
                free_gb: 0.0,
                usage_percent: 0.0,
                disk_name: "unknown".to_string(),
                mount_point: "/".to_string(),
            }
        });
        let system_info = self.get_system_info();

        SystemMetrics {
            timestamp,
            cpu_usage_percent: cpu_usage,
            memory_usage: memory,
            disk_usage: disk,
            system_info,
        }
    }

    /// Get quick system health score (0-100, higher is better)
    pub fn get_health_score(&mut self) -> u8 {
        let cpu_usage = self.get_cpu_usage();
        let memory = self.get_memory_usage();
        let disk = self.get_disk_usage();

        let cpu_score = if cpu_usage < 50.0 {
            25
        } else if cpu_usage < 80.0 {
            15
        } else {
            5
        };
        let memory_score = if memory.usage_percent < 50.0 {
            25
        } else if memory.usage_percent < 80.0 {
            15
        } else {
            5
        };
        let disk_score = if let Some(disk_data) = disk {
            if disk_data.usage_percent < 70.0 {
                25
            } else if disk_data.usage_percent < 90.0 {
                15
            } else {
                5
            }
        } else {
            20 // Default score if disk info unavailable
        };

        let base_score = 25; // System uptime and process health

        (cpu_score + memory_score + disk_score + base_score) as u8
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
