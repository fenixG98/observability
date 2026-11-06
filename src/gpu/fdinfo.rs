//! Per-process GPU usage from `/proc/<pid>/fdinfo` (DRM drivers: i915, amdgpu, ...).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    read_clients(&list_drm_fds(root))
}

/// Incremental scan. Finding the DRM fds needs a `readlink` for every fd of every process
/// (thousands); reading the fdinfo of the already known ones, however, is cheap. So the full
/// list is rebuilt every `rescan_every` and, in between, only the known ones are re-read.
#[derive(Debug, Default)]
pub struct DrmScanner {
    known: Vec<(u32, PathBuf)>,
    scanned_at: Option<Instant>,
}

impl DrmScanner {
    pub fn scan(&mut self, root: &Path, rescan_every: Duration) -> Vec<(u32, DrmClient)> {
        if self.scanned_at.is_none_or(|t| t.elapsed() >= rescan_every) {
            self.known = list_drm_fds(root);
            self.scanned_at = Some(Instant::now());
        }
        // Fds that are no longer DRM (closed or reused for another file) are discarded.
        let mut clients = Vec::new();
        self.known.retain(|(pid, info)| match read_client(info) {
            Some(c) => {
                clients.push((*pid, c));
                true
            }
            None => false,
        });
        clients
    }
}

fn read_client(info: &Path) -> Option<DrmClient> {
    parse_fdinfo(&fs::read_to_string(info).ok()?)
}

fn read_clients(fds: &[(u32, PathBuf)]) -> Vec<(u32, DrmClient)> {
    fds.iter()
        .filter_map(|(pid, info)| read_client(info).map(|c| (*pid, c)))
        .collect()
}

/// (pid, fdinfo path) of every fd that points to `/dev/dri/`.
fn list_drm_fds(root: &Path) -> Vec<(u32, PathBuf)> {
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
            out.push((pid, entry.path().join("fdinfo").join(fd.file_name())));
        }
    }
    out
}
