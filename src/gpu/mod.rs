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
