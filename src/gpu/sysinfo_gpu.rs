//! Generic provider via `sysinfo::Gpus` (AMD on Linux, Apple and discrete GPUs on macOS).

use sysinfo::Gpus;

use crate::gpu::GpuProvider;
use crate::model::{Avail, GpuReading};

/// `false` if the vendor is already handled by a specific provider (`handled`, lowercase).
pub fn vendor_included(vendor: Option<&str>, handled: &[&str]) -> bool {
    let v = vendor.unwrap_or("").to_lowercase();
    !handled.iter().any(|h| v.contains(h))
}

/// Reason for the user when sysinfo does not give the usage.
pub fn unavailable_reason(vendor: Option<&str>) -> String {
    let v = vendor.unwrap_or("desconegut");
    let l = v.to_lowercase();
    if l.contains("nvidia") {
        "NVML no disponible: cal el driver propietari de NVIDIA (libnvidia-ml)".into()
    } else if l.contains("amd") || l.contains("advanced micro") {
        "el driver amdgpu no exposa gpu_busy_percent per a aquesta GPU".into()
    } else {
        format!("el sistema no exposa l'ús d'aquesta GPU ({v})")
    }
}

/// Model of the first GPU whose vendor contains `needle` (lowercase).
pub fn model_for_vendor(needle: &str) -> Option<String> {
    let gpus = Gpus::new_with_refreshed_list().ok()?;
    (&gpus)
        .into_iter()
        .find(|g| g.vendor().unwrap_or("").to_lowercase().contains(needle))
        .and_then(|g| g.model().map(str::to_string))
}

pub struct SysinfoGpu {
    gpus: Gpus,
    handled: Vec<String>,
}

impl SysinfoGpu {
    /// `None` if there is no GPU left to manage.
    pub fn new(handled: &[&str]) -> Option<SysinfoGpu> {
        let gpus = Gpus::new_with_refreshed_list().ok()?;
        let any = (&gpus)
            .into_iter()
            .any(|g| vendor_included(g.vendor(), handled));
        any.then(|| SysinfoGpu {
            gpus,
            handled: handled.iter().map(|h| h.to_string()).collect(),
        })
    }
}

impl GpuProvider for SysinfoGpu {
    fn read(&mut self) -> Vec<GpuReading> {
        self.gpus.refresh(false);
        let handled: Vec<&str> = self.handled.iter().map(String::as_str).collect();
        let shared_memory = cfg!(target_os = "macos");
        (&self.gpus)
            .into_iter()
            .filter(|g| vendor_included(g.vendor(), &handled))
            .map(|g| {
                let vendor = g.vendor().unwrap_or("").to_lowercase();
                GpuReading {
                    name: g.model().unwrap_or("GPU").to_string(),
                    discrete: !shared_memory && !vendor.contains("intel"),
                    util: match g.usage() {
                        Some(u) => Avail::Ok(u),
                        None => Avail::Unavailable(unavailable_reason(g.vendor())),
                    },
                    vram_used: if shared_memory { None } else { g.used_memory() },
                    vram_total: if shared_memory {
                        None
                    } else {
                        g.total_memory()
                    },
                    temp_c: None,
                    power_w: None,
                    engines: Vec::new(),
                }
            })
            .collect()
    }
}
