use mindbox_common::{CpuInfo, GpuInfo, MemoryInfo, SystemResources};
use nvml_wrapper::Nvml;
use sysinfo::System;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct SystemMonitorService {
    system: Mutex<System>,
    nvml: Option<Nvml>,
}

impl SystemMonitorService {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        system.refresh_memory();

        Self {
            system: Mutex::new(system),
            nvml: Nvml::init().ok(),
        }
    }

    pub async fn snapshot(&self) -> SystemResources {
        let (cpu, memory) = {
            let mut system = self.system.lock().await;
            system.refresh_cpu_usage();
            system.refresh_memory();

            let cpu_usage = clamp_percent(system.global_cpu_usage());
            let memory_used = system.used_memory();
            let memory_total = system.total_memory();
            let memory_usage = percent_from_ratio(memory_used, memory_total);

            (
                CpuInfo {
                    utilization_percent: cpu_usage,
                },
                MemoryInfo {
                    used_bytes: memory_used,
                    total_bytes: memory_total,
                    utilization_percent: memory_usage,
                },
            )
        };

        let gpus = self.snapshot_gpus();

        SystemResources { cpu, memory, gpus }
    }

    fn snapshot_gpus(&self) -> Vec<GpuInfo> {
        let Some(nvml) = &self.nvml else {
            return Vec::new();
        };

        let Ok(device_count) = nvml.device_count() else {
            return Vec::new();
        };

        let mut gpus = Vec::new();
        for index in 0..device_count {
            let Ok(device) = nvml.device_by_index(index) else {
                continue;
            };

            let name = device.name().unwrap_or_else(|_| format!("GPU-{index}"));
            let utilization_percent = device
                .utilization_rates()
                .map(|utilization| clamp_percent(utilization.gpu as f32))
                .unwrap_or(0.0);

            let (memory_used_bytes, memory_total_bytes, memory_utilization_percent) =
                match device.memory_info() {
                    Ok(memory) => {
                        let usage = percent_from_ratio(memory.used, memory.total);
                        (memory.used, memory.total, usage)
                    }
                    Err(_) => (0, 0, 0.0),
                };

            gpus.push(GpuInfo {
                name,
                utilization_percent,
                memory_used_bytes,
                memory_total_bytes,
                memory_utilization_percent,
            });
        }

        gpus
    }
}

fn percent_from_ratio(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        clamp_percent((used as f64 * 100.0 / total as f64) as f32)
    }
}

fn clamp_percent(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 100.0)
    }
}
