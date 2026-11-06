//! Intel GPU usage (i915 driver) via the perf PMU.
//! Requires CAP_PERFMON on the binary (or `perf_event_paranoid <= 0`). Without permission, there is no
//! partial figure: the error is shown with the exact command to fix it.

use std::fs;
use std::io::{self, Read};
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::gpu::GpuProvider;
use crate::model::{Avail, GpuReading};

/// "config=0x2010" (or "event=..,config=12") → `config` value.
pub fn parse_event_config(text: &str) -> Option<u64> {
    text.trim()
        .split(',')
        .find_map(|kv| kv.trim().strip_prefix("config="))
        .and_then(|v| {
            let v = v.trim();
            match v.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16).ok(),
                None => v.parse().ok(),
            }
        })
}

/// "rcs0-busy" → "render", "vcs1-busy" → "video", ...
pub fn engine_label(event: &str) -> String {
    let base = event.trim_end_matches("-busy");
    let class: String = base
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    match class.as_str() {
        "rcs" => "render".into(),
        "vcs" => "video".into(),
        "vecs" => "video-enh".into(),
        "bcs" => "copy".into(),
        "ccs" => "compute".into(),
        _ => base.to_string(),
    }
}

pub fn permission_message(exe: &Path) -> String {
    format!(
        "permission needed to read the Intel GPU: sudo setcap cap_perfmon=ep {} (lost on rebuild or reinstall)",
        exe.display()
    )
}

/// Translates the `perf_event_open` error into a user-facing message.
pub fn open_error_reason(err: &io::Error, exe: &Path) -> String {
    match err.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => permission_message(exe),
        Some(libc::ENOENT) | Some(libc::ENODEV) => format!("i915 PMU not available ({err})"),
        _ => format!("error opening the i915 PMU: {err}"),
    }
}

/// i915 PMU directory inside `root` (`/sys/bus/event_source/devices`).
pub fn find_pmu(root: &Path) -> Option<PathBuf> {
    let exact = root.join("i915");
    if exact.is_dir() {
        return Some(exact);
    }
    let mut found: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("i915"))
        .map(|e| e.path())
        .collect();
    found.sort();
    found.into_iter().next()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmuEvent {
    pub name: String,
    pub config: u64,
}

/// The PMU's `*-busy` events.
pub fn list_busy_events(pmu: &Path) -> Vec<PmuEvent> {
    let Ok(entries) = fs::read_dir(pmu.join("events")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with("-busy") {
                return None;
            }
            let config = parse_event_config(&fs::read_to_string(e.path()).ok()?)?;
            Some(PmuEvent { name, config })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntelCard {
    pub card: String,
    pub driver: String,
}

/// First DRM card with an Intel vendor (0x8086) inside `drm_root` (`/sys/class/drm`).
pub fn find_intel_card(drm_root: &Path) -> Option<IntelCard> {
    let mut cards: Vec<String> = fs::read_dir(drm_root)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("card") && !n.contains('-'))
        .collect();
    cards.sort();
    for card in cards {
        let dev = drm_root.join(&card).join("device");
        let vendor = fs::read_to_string(dev.join("vendor")).unwrap_or_default();
        if vendor.trim() != "0x8086" {
            continue;
        }
        let driver = fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_default();
        return Some(IntelCard { card, driver });
    }
    None
}

/// `perf_event_attr` at the minimum size the kernel accepts (PERF_ATTR_SIZE_VER0 = 64 bytes).
/// The field order is the kernel's: it cannot be reordered.
#[repr(C)]
#[derive(Default)]
struct PerfEventAttr {
    type_: u32,
    size: u32,
    config: u64,
    sample_period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup_events: u32,
    bp_type: u32,
    config1: u64,
}

fn perf_open(pmu_type: u32, config: u64, cpu: i32) -> io::Result<fs::File> {
    let attr = PerfEventAttr {
        type_: pmu_type,
        size: std::mem::size_of::<PerfEventAttr>() as u32,
        config,
        ..Default::default()
    };
    // SAFETY: `attr` is valid for the duration of the call; pid = -1 and cpu >= 0 is what an "uncore" PMU requires.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const PerfEventAttr,
            -1 as libc::pid_t,
            cpu as libc::c_int,
            -1 as libc::c_int,
            0 as libc::c_ulong,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the kernel gave us a new fd that is now ours.
    Ok(unsafe { fs::File::from_raw_fd(fd as i32) })
}

fn read_counter(f: &fs::File) -> io::Result<u64> {
    let mut b = [0u8; 8];
    (&*f).read_exact(&mut b)?;
    Ok(u64::from_ne_bytes(b))
}

struct Counter {
    label: String,
    file: fs::File,
    prev: u64,
}

enum IntelState {
    Ready {
        counters: Vec<Counter>,
        last: Instant,
    },
    Failed(String),
}

pub struct IntelGpu {
    name: String,
    state: IntelState,
}

impl IntelGpu {
    /// `None` if there is no Intel GPU. If there is one but it cannot be read, returns a
    /// provider in an error state (to show the reason in the UI).
    pub fn detect(name: &str) -> Option<IntelGpu> {
        let card = find_intel_card(Path::new("/sys/class/drm"))?;
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("guaitop"));
        let state = if card.driver == "i915" {
            match find_pmu(Path::new("/sys/bus/event_source/devices")) {
                Some(pmu) => open_counters(&pmu, &exe),
                None => {
                    IntelState::Failed("i915 PMU not found in /sys/bus/event_source/devices".into())
                }
            }
        } else {
            IntelState::Failed(format!(
                "driver {}: usage reading not supported yet (i915 only)",
                if card.driver.is_empty() {
                    "unknown"
                } else {
                    &card.driver
                }
            ))
        };
        Some(IntelGpu {
            name: name.to_string(),
            state,
        })
    }

    /// Provider that always reports an error (also useful for tests).
    pub fn failed(name: &str, reason: &str) -> IntelGpu {
        IntelGpu {
            name: name.to_string(),
            state: IntelState::Failed(reason.to_string()),
        }
    }

    fn reading(&self, util: Avail<f32>, engines: Vec<(String, f32)>) -> GpuReading {
        GpuReading {
            name: self.name.clone(),
            discrete: false,
            util,
            vram_used: None,
            vram_total: None,
            temp_c: None,
            power_w: None,
            engines,
        }
    }
}

fn open_counters(pmu: &Path, exe: &Path) -> IntelState {
    let Some(pmu_type) = fs::read_to_string(pmu.join("type"))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return IntelState::Failed("could not read the i915 PMU type".into());
    };
    let cpu = fs::read_to_string(pmu.join("cpumask"))
        .ok()
        .and_then(|s| {
            s.trim()
                .split([',', '-'])
                .next()
                .and_then(|c| c.parse::<i32>().ok())
        })
        .unwrap_or(0);
    let mut counters = Vec::new();
    let mut first_err: Option<io::Error> = None;
    for ev in list_busy_events(pmu) {
        match perf_open(pmu_type, ev.config, cpu) {
            Ok(file) => {
                let prev = read_counter(&file).unwrap_or(0);
                counters.push(Counter {
                    label: engine_label(&ev.name),
                    file,
                    prev,
                });
            }
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if counters.is_empty() {
        return IntelState::Failed(match first_err {
            Some(e) => open_error_reason(&e, exe),
            None => "the i915 PMU exposes no *-busy counter".into(),
        });
    }
    IntelState::Ready {
        counters,
        last: Instant::now(),
    }
}

impl GpuProvider for IntelGpu {
    fn read(&mut self) -> Vec<GpuReading> {
        let (util, engines) = match &mut self.state {
            IntelState::Failed(reason) => (Avail::Unavailable(reason.clone()), Vec::new()),
            IntelState::Ready { counters, last } => {
                let now = Instant::now();
                let dt = now.duration_since(*last).as_nanos().max(1) as f64;
                *last = now;
                let mut engines: Vec<(String, f32)> = Vec::new();
                for c in counters.iter_mut() {
                    let value = read_counter(&c.file).unwrap_or(c.prev);
                    let busy =
                        (value.saturating_sub(c.prev) as f64 / dt * 100.0).clamp(0.0, 100.0) as f32;
                    c.prev = value;
                    match engines.iter_mut().find(|(l, _)| *l == c.label) {
                        Some(e) => e.1 = e.1.max(busy),
                        None => engines.push((c.label.clone(), busy)),
                    }
                }
                let total = engines.iter().map(|e| e.1).fold(0.0, f32::max);
                (Avail::Ok(total), engines)
            }
        };
        vec![self.reading(util, engines)]
    }
}
