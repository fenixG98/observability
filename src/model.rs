//! Types shared between collectors, application state and UI.

/// A piece of data that may be unavailable, with the reason (for the user).
#[derive(Debug, Clone, PartialEq)]
pub enum Avail<T> {
    Ok(T),
    Unavailable(String),
}

impl<T> Avail<T> {
    pub fn ok(&self) -> Option<&T> {
        match self {
            Avail::Ok(v) => Some(v),
            Avail::Unavailable(_) => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Avail::Ok(_) => None,
            Avail::Unavailable(r) => Some(r),
        }
    }
}

impl<T> Default for Avail<T> {
    fn default() -> Self {
        Avail::Unavailable("sense dades encara".into())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostInfo {
    pub hostname: String,
    pub uptime_secs: u64,
    pub load: [f64; 3],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuSnap {
    pub model: String,
    /// Total usage, 0..=100.
    pub total: f32,
    /// Usage per logical thread, 0..=100.
    pub per_core: Vec<f32>,
    /// Average frequency in MHz (0 if unknown).
    pub freq_mhz: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemKind {
    /// Memory in use that cannot easily be freed (apps, wired, compressed).
    Used,
    /// Memory in use but reclaimable (cache, buffers).
    Reclaimable,
    Free,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemSegment {
    pub label: String,
    pub bytes: u64,
    pub kind: MemKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemSnap {
    pub total: u64,
    pub segments: Vec<MemSegment>,
    pub swap_used: u64,
    pub swap_total: u64,
}

impl MemSnap {
    /// Fraction 0..=1 of `Used` memory (the part that pressures the system).
    pub fn used_fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let used: u64 = self
            .segments
            .iter()
            .filter(|s| s.kind == MemKind::Used)
            .map(|s| s.bytes)
            .sum();
        (used as f64 / self.total as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuReading {
    pub name: String,
    pub discrete: bool,
    /// Usage 0..=100, or the reason why it cannot be read.
    pub util: Avail<f32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    /// (engine, usage 0..=100), e.g. ("render", 42.0).
    pub engines: Vec<(String, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempSensor {
    pub label: String,
    pub celsius: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetSnap {
    /// What is being measured: "enp0s31f6", "2 physical", "eth9 (not found)".
    pub iface_label: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MountUsage {
    pub mount: String,
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskSnap {
    /// Device of the main mount point.
    pub device: String,
    /// The first is the main one (`/` if it exists).
    pub mounts: Vec<MountUsage>,
    pub read_bps: u64,
    pub write_bps: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    /// % of one CPU (can exceed 100 with several threads).
    pub cpu: f32,
    pub mem: u64,
    /// GPU %, `None` if unknown.
    pub gpu: Option<f32>,
    pub threads: Option<usize>,
    /// Seconds since the Unix epoch.
    pub start_time: u64,
    pub exe: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Seconds since the Unix epoch at which the sample was taken.
    pub taken_at: u64,
    pub host: HostInfo,
    pub cpu: CpuSnap,
    pub mem: MemSnap,
    pub gpus: Avail<Vec<GpuReading>>,
    pub temps: Avail<Vec<TempSensor>>,
    pub net: NetSnap,
    pub disk: DiskSnap,
    /// `Some` only when the processes tab is visible.
    pub procs: Option<Vec<ProcInfo>>,
}

const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

/// Size in compact binary units: "512B", "2K", "340M", "1.8G".
pub fn human_bytes(b: u64) -> String {
    let f = b as f64;
    if f >= GIB {
        format!("{:.1}G", f / GIB)
    } else if f >= MIB {
        format!("{:.0}M", f / MIB)
    } else if f >= KIB {
        format!("{:.0}K", f / KIB)
    } else {
        format!("{b}B")
    }
}

/// Speed in decimal units: "500 B/s", "1.1 KB/s", "12.4 MB/s".
pub fn human_rate(bps: u64) -> String {
    let f = bps as f64;
    if f >= 1e9 {
        format!("{:.1} GB/s", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1} MB/s", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1} KB/s", f / 1e3)
    } else {
        format!("{bps} B/s")
    }
}

/// Duration with the two largest units: "3d 4h", "4h 12m", "12m", "42s".
pub fn human_duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}
