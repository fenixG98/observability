//! NVIDIA provider via NVML (loaded dynamically; without a driver, `new()` fails without panicking).

use std::collections::HashMap;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;

use crate::gpu::GpuProvider;
use crate::model::{Avail, GpuReading};

pub struct NvidiaGpu {
    nvml: Nvml,
    count: u32,
    last_seen: u64,
}

impl NvidiaGpu {
    pub fn new() -> Result<NvidiaGpu, String> {
        let nvml = Nvml::init().map_err(|e| e.to_string())?;
        let count = nvml.device_count().map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("no NVIDIA device".into());
        }
        Ok(NvidiaGpu {
            nvml,
            count,
            last_seen: 0,
        })
    }
}

impl GpuProvider for NvidiaGpu {
    fn read(&mut self) -> Vec<GpuReading> {
        (0..self.count)
            .map(|i| match self.nvml.device_by_index(i) {
                Err(e) => GpuReading {
                    name: format!("NVIDIA #{i}"),
                    discrete: true,
                    util: Avail::Unavailable(format!("NVML: {e}")),
                    vram_used: None,
                    vram_total: None,
                    temp_c: None,
                    power_w: None,
                    engines: Vec::new(),
                },
                Ok(d) => {
                    let mem = d.memory_info().ok();
                    GpuReading {
                        name: d.name().unwrap_or_else(|_| format!("NVIDIA #{i}")),
                        discrete: true,
                        util: match d.utilization_rates() {
                            Ok(u) => Avail::Ok(u.gpu as f32),
                            Err(e) => Avail::Unavailable(format!("NVML: {e}")),
                        },
                        vram_used: mem.as_ref().map(|m| m.used),
                        vram_total: mem.as_ref().map(|m| m.total),
                        temp_c: d.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f32),
                        power_w: d.power_usage().ok().map(|mw| mw as f32 / 1000.0),
                        engines: Vec::new(),
                    }
                }
            })
            .collect()
    }

    fn process_usage(&mut self) -> HashMap<u32, f32> {
        let mut out: HashMap<u32, f32> = HashMap::new();
        let mut newest = self.last_seen;
        for i in 0..self.count {
            let Ok(d) = self.nvml.device_by_index(i) else {
                continue;
            };
            let Ok(samples) = d.process_utilization_stats(self.last_seen) else {
                continue;
            };
            for s in samples {
                newest = newest.max(s.timestamp);
                *out.entry(s.pid).or_insert(0.0) += s.sm_util as f32;
            }
        }
        self.last_seen = newest;
        out
    }
}
