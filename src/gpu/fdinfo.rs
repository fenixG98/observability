//! Per-process GPU usage from `/proc/<pid>/fdinfo` (DRM drivers: i915, amdgpu, ...).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Accumulated counters of a DRM client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrmClient {
    pub client_id: String,
    pub pdev: String,
    /// Engine → nanoseconds busy (cumulative).
    pub engines: HashMap<String, u64>,
}

pub fn parse_fdinfo(text: &str) -> Option<DrmClient> {
    let mut c = DrmClient::default();
    let mut has_driver = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "drm-driver" => has_driver = true,
            "drm-client-id" => c.client_id = value.to_string(),
            "drm-pdev" => c.pdev = value.to_string(),
            _ => {
                if let Some(engine) = key.strip_prefix("drm-engine-")
                    && !engine.starts_with("capacity-")
                    && let Some(ns) = value
                        .strip_suffix("ns")
                        .and_then(|n| n.trim().parse::<u64>().ok())
                {
                    c.engines.insert(engine.to_string(), ns);
                }
            }
        }
    }
    (has_driver && !c.client_id.is_empty()).then_some(c)
}

/// Converts successive samples into per-process % (the busiest engine, max 100).
#[derive(Debug, Default)]
pub struct FdinfoTracker {
    prev: HashMap<(u32, String), HashMap<String, u64>>,
}

impl FdinfoTracker {
    pub fn update(&mut self, samples: Vec<(u32, DrmClient)>, elapsed_ns: u64) -> HashMap<u32, f32> {
        let mut out: HashMap<u32, f32> = HashMap::new();
        let mut next: HashMap<(u32, String), HashMap<String, u64>> = HashMap::new();
        for (pid, client) in samples {
            let key = (pid, client.client_id.clone());
            if next.contains_key(&key) {
                continue;
            }
            if elapsed_ns > 0
                && let Some(prev) = self.prev.get(&key)
            {
                let busiest = client
                    .engines
                    .iter()
                    .map(|(engine, ns)| {
                        let before = prev.get(engine).copied().unwrap_or(*ns);
                        ns.saturating_sub(before) as f64 / elapsed_ns as f64 * 100.0
                    })
                    .fold(0.0_f64, f64::max)
                    .min(100.0) as f32;
                let entry = out.entry(pid).or_insert(0.0);
                *entry = entry.max(busiest);
            }
            next.insert(key, client.engines);
        }
        self.prev = next;
        out
    }
}

/// Walks `root` (normally `/proc`) and reads the fdinfo of the fds that point to `/dev/dri/`.
/// Only sees the processes the user can read.
pub fn scan_proc(root: &Path) -> Vec<(u32, DrmClient)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let is_drm = fs::read_link(fd.path()).is_ok_and(|t| t.starts_with("/dev/dri/"));
            if !is_drm {
                continue;
            }
            let info = entry.path().join("fdinfo").join(fd.file_name());
            if let Some(client) = fs::read_to_string(info)
                .ok()
                .as_deref()
                .and_then(parse_fdinfo)
            {
                out.push((pid, client));
            }
        }
    }
    out
}
