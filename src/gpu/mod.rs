//! GPU providers. Each provider manages one or more devices.

use std::collections::HashMap;

use crate::model::GpuReading;

pub mod fdinfo;
#[cfg(target_os = "linux")]
pub mod intel;
#[cfg(target_os = "linux")]
pub mod nvidia;
pub mod sysinfo_gpu;

/// Source of GPU readings. Created and used inside the collector thread.
pub trait GpuProvider {
    /// One reading per managed device.
    fn read(&mut self) -> Vec<GpuReading>;

    /// Per-process GPU usage (pid → %), if this provider knows it.
    fn process_usage(&mut self) -> HashMap<u32, f32> {
        HashMap::new()
    }
}

/// Detects the providers available on this machine.
/// Linux: NVML (NVIDIA), i915 PMU (Intel) and sysinfo for the rest (AMD...).
/// macOS: sysinfo (Apple Silicon / discrete GPU on Intel Macs).
pub fn detect() -> Vec<Box<dyn GpuProvider>> {
    let mut out: Vec<Box<dyn GpuProvider>> = Vec::new();
    #[cfg(target_os = "linux")]
    let handled = linux_providers(&mut out);
    #[cfg(not(target_os = "linux"))]
    let handled: Vec<&'static str> = Vec::new();
    if let Some(s) = sysinfo_gpu::SysinfoGpu::new(&handled) {
        out.push(Box::new(s));
    }
    out
}

#[cfg(target_os = "linux")]
fn linux_providers(out: &mut Vec<Box<dyn GpuProvider>>) -> Vec<&'static str> {
    let mut handled = Vec::new();
    if let Ok(n) = nvidia::NvidiaGpu::new() {
        out.push(Box::new(n));
        handled.push("nvidia");
    }
    let intel_name =
        sysinfo_gpu::model_for_vendor("intel").unwrap_or_else(|| "Intel GPU".to_string());
    if let Some(i) = intel::IntelGpu::detect(&intel_name) {
        out.push(Box::new(i));
        handled.push("intel");
    }
    handled
}
