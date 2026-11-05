//! Direct `/proc` readers (Linux): a single file read instead of the many `/sys` files
//! that sysinfo reads for network and disk.

use std::collections::HashMap;

/// `/proc/net/dev` → (interface, bytes received, bytes sent), in file order.
pub fn parse_net_dev(text: &str) -> Vec<(String, u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let f: Vec<u64> = rest
                .split_whitespace()
                .map(|x| x.parse().ok())
                .collect::<Option<_>>()?;
            (f.len() >= 9).then(|| (name.trim().to_string(), f[0], f[8]))
        })
        .collect()
}

/// `/proc/diskstats` → device → (bytes read, bytes written). 512-byte sectors.
pub fn parse_diskstats(text: &str) -> HashMap<String, (u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 10 {
                return None;
            }
            let read = f[5].parse::<u64>().ok()?;
            let written = f[9].parse::<u64>().ok()?;
            Some((f[2].to_string(), (read * 512, written * 512)))
        })
        .collect()
}

/// Converts cumulative counters into increments since the previous sample.
#[derive(Debug, Default)]
pub struct Deltas {
    prev: HashMap<String, (u64, u64)>,
}

impl Deltas {
    /// Increment of (a, b) for `name`. On the first sample, or if the counter went down
    /// (it was reset), returns (0, 0).
    pub fn update(&mut self, name: &str, now: (u64, u64)) -> (u64, u64) {
        let delta = match self.prev.get(name) {
            Some(p) if now.0 >= p.0 && now.1 >= p.1 => (now.0 - p.0, now.1 - p.1),
            _ => (0, 0),
        };
        self.prev.insert(name.to_string(), now);
        delta
    }
}
