//! Real data source: sysinfo + GPU providers + platform parsers.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{
    Components, Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users,
};

use crate::collect::procfs::Deltas;
use crate::collect::{Control, Source, netfilter, temps};
use crate::gpu::GpuProvider;
use crate::gpu::fdinfo::FdinfoTracker;
use crate::model::*;

#[cfg(target_os = "macos")]
const NO_SENSORS: &str = "could not read the sensors (IOHID/SMC)";
#[cfg(not(target_os = "macos"))]
const NO_SENSORS: &str = "no hwmon sensor found (load the sensor module, e.g. coretemp or k10temp)";

/// Temperatures change slowly and some sensors are slow to read (the NVMe one
/// does a SMART query, the ACPI one evaluates firmware code): they are read every 5 s.
const TEMPS_EVERY: Duration = Duration::from_secs(5);
/// Used space and the list of mount points: every 10 s. I/O, on every sample.
const DISK_SPACE_EVERY: Duration = Duration::from_secs(10);
/// How often to look for new processes using the GPU (see `DrmScanner`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const DRM_RESCAN_EVERY: Duration = Duration::from_secs(5);

/// Raw sensor reading: label and temperature (if it gave one).
type RawTemp = (String, Option<f32>);

pub struct SystemSource {
    sys: System,
    nets: Option<Networks>,
    disks: Option<Disks>,
    comps: Option<Components>,
    users: Option<Users>,
    users_at: Instant,
    gpus: Vec<Box<dyn GpuProvider>>,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fdinfo: FdinfoTracker,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    drm: crate::gpu::fdinfo::DrmScanner,
    last: Instant,
    net_deltas: Deltas,
    disk_deltas: Deltas,
    /// Last space reading: root device, mounts and devices (`/proc/diskstats`
    /// names) to sum for I/O.
    disk_space: Option<(Instant, String, Vec<MountUsage>, Vec<String>)>,
    /// Raw sensor reading and when it was taken.
    temps_raw: Option<(Instant, Vec<RawTemp>)>,
}

impl SystemSource {
    pub fn new(gpus: Vec<Box<dyn GpuProvider>>) -> SystemSource {
        let mut sys = System::new().expect("sysinfo does not support this OS");
        sys.refresh_cpu_all();
        SystemSource {
            sys,
            // On Linux the network is read from /proc/net/dev; sysinfo is only plan B.
            nets: (!cfg!(target_os = "linux") || !std::path::Path::new("/proc/net/dev").exists())
                .then(|| Networks::new_with_refreshed_list().ok())
                .flatten(),
            disks: Disks::new_with_refreshed_list().ok(),
            comps: Components::new_with_refreshed_list().ok(),
            users: Users::new_with_refreshed_list().ok(),
            users_at: Instant::now(),
            gpus,
            fdinfo: FdinfoTracker::default(),
            drm: Default::default(),
            last: Instant::now(),
            net_deltas: Deltas::default(),
            disk_deltas: Deltas::default(),
            disk_space: None,
            temps_raw: None,
        }
    }

    fn host() -> HostInfo {
        HostInfo {
            hostname: System::host_name().unwrap_or_default(),
            uptime_secs: System::uptime().unwrap_or(0),
            load: System::load_average()
                .map(|l| [l.one, l.five, l.fifteen])
                .unwrap_or_default(),
        }
    }

    fn cpu(&mut self) -> CpuSnap {
        self.sys.refresh_cpu_all();
        let cpus = self.sys.cpus();
        let freqs: Vec<u64> = cpus
            .iter()
            .map(|c| c.frequency())
            .filter(|f| *f > 0)
            .collect();
        CpuSnap {
            model: cpus
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
            total: self.sys.global_cpu_usage(),
            per_core: cpus.iter().map(|c| c.usage()).collect(),
            freq_mhz: if freqs.is_empty() {
                0
            } else {
                freqs.iter().sum::<u64>() / freqs.len() as u64
            },
        }
    }

    fn generic_mem(&self) -> MemSnap {
        let total = self.sys.total_memory();
        let used = self.sys.used_memory().min(total);
        MemSnap {
            total,
            segments: vec![
                MemSegment {
                    label: "used".into(),
                    bytes: used,
                    kind: MemKind::Used,
                },
                MemSegment {
                    label: "free".into(),
                    bytes: total - used,
                    kind: MemKind::Free,
                },
            ],
            swap_used: self.sys.used_swap(),
            swap_total: self.sys.total_swap(),
        }
    }

    fn mem(&mut self) -> MemSnap {
        self.sys.refresh_memory();
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/meminfo")
                .map_err(|e| e.to_string())
                .and_then(|t| crate::collect::meminfo::parse_linux_meminfo(&t))
                .unwrap_or_else(|_| self.generic_mem())
        }
        #[cfg(target_os = "macos")]
        {
            crate::collect::meminfo::read_macos(
                self.sys.total_memory(),
                self.sys.used_swap(),
                self.sys.total_swap(),
            )
            .unwrap_or_else(|_| self.generic_mem())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            self.generic_mem()
        }
    }

    fn net(&mut self, ctl: &Control, dt: f64) -> NetSnap {
        #[cfg(target_os = "linux")]
        if let Ok(text) = std::fs::read_to_string("/proc/net/dev") {
            let counters = crate::collect::procfs::parse_net_dev(&text);
            let names: Vec<String> = counters.iter().map(|c| c.0.clone()).collect();
            let sel = netfilter::select(&names, ctl.net_iface().as_deref());
            let (mut rx, mut tx) = (0u64, 0u64);
            // Increments are computed for all of them: changing the selection causes no jumps.
            for (name, r, t) in &counters {
                let (dr, dtx) = self.net_deltas.update(name, (*r, *t));
                if sel.names.iter().any(|s| s == name) {
                    rx += dr;
                    tx += dtx;
                }
            }
            return NetSnap {
                iface_label: sel.label,
                rx_bps: (rx as f64 / dt) as u64,
                tx_bps: (tx as f64 / dt) as u64,
            };
        }
        let Some(nets) = self.nets.as_mut() else {
            return NetSnap {
                iface_label: "network not available".into(),
                ..Default::default()
            };
        };
        nets.refresh(true);
        let names: Vec<String> = nets
            .keys()
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        let sel = netfilter::select(&names, ctl.net_iface().as_deref());
        let (mut rx, mut tx) = (0u64, 0u64);
        for (name, data) in nets.iter() {
            if sel
                .names
                .iter()
                .any(|s| s.as_str() == name.to_string_lossy())
            {
                rx += data.received();
                tx += data.transmitted();
            }
        }
        NetSnap {
            iface_label: sel.label,
            rx_bps: (rx as f64 / dt) as u64,
            tx_bps: (tx as f64 / dt) as u64,
        }
    }

    /// Space and I/O. On Linux, space (which needs a `statvfs` per mount) is refreshed every
    /// [`DISK_SPACE_EVERY`] and I/O comes from `/proc/diskstats`, a single read.
    fn disk(&mut self, dt: f64) -> DiskSnap {
        #[cfg(target_os = "linux")]
        {
            let stale = self
                .disk_space
                .as_ref()
                .is_none_or(|(at, ..)| at.elapsed() >= DISK_SPACE_EVERY);
            if stale && let Some(d) = self.disks.as_mut() {
                d.refresh_specifics(true, sysinfo::DiskRefreshKind::nothing().with_storage());
                let snap = Self::disk_snapshot(d, 0.0);
                let devs = snap_devices(d);
                self.disk_space = Some((Instant::now(), snap.device, snap.mounts, devs));
            }
            if let (Some((_, device, mounts, devs)), Ok(text)) = (
                self.disk_space.as_ref(),
                std::fs::read_to_string("/proc/diskstats"),
            ) {
                let stats = crate::collect::procfs::parse_diskstats(&text);
                let (mut read, mut written) = (0u64, 0u64);
                for dev in devs {
                    if let Some(now) = stats.get(dev) {
                        let (r, w) = self.disk_deltas.update(dev, *now);
                        read += r;
                        written += w;
                    }
                }
                return DiskSnap {
                    device: device.clone(),
                    mounts: mounts.clone(),
                    read_bps: (read as f64 / dt) as u64,
                    write_bps: (written as f64 / dt) as u64,
                };
            }
        }
        let Some(disks) = self.disks.as_mut() else {
            return DiskSnap::default();
        };
        disks.refresh(true);
        Self::disk_snapshot(disks, dt)
    }

    fn disk_snapshot(disks: &Disks, dt: f64) -> DiskSnap {
        let mut mounts: Vec<MountUsage> = Vec::new();
        let mut seen_devices: Vec<String> = Vec::new();
        let (mut read, mut written) = (0u64, 0u64);
        let mut device = String::new();
        for d in disks.iter() {
            let mount = d.mount_point().display().to_string();
            let dev = d.name().to_string_lossy().into_owned();
            if d.total_space() == 0 || mounts.iter().any(|m| m.mount == mount) {
                continue;
            }
            if mount == "/" {
                device = dev.clone();
            }
            mounts.push(MountUsage {
                mount,
                used: d.total_space().saturating_sub(d.available_space()),
                total: d.total_space(),
            });
            if !seen_devices.contains(&dev) {
                let u = d.usage();
                read += u.read_bytes;
                written += u.written_bytes;
                seen_devices.push(dev);
            }
        }
        mounts.sort_by(|a, b| (a.mount != "/", &a.mount).cmp(&(b.mount != "/", &b.mount)));
        if device.is_empty() {
            device = seen_devices.first().cloned().unwrap_or_default();
        }
        let rate = |b: u64| if dt > 0.0 { (b as f64 / dt) as u64 } else { 0 };
        DiskSnap {
            device,
            mounts,
            read_bps: rate(read),
            write_bps: rate(written),
        }
    }

    /// Temperatures. Sensors are read every [`TEMPS_EVERY`], and are not read
    /// while the processes tab is visible (they are not shown there).
    fn temps(&mut self, gpus: &[GpuReading], visible: bool) -> Avail<Vec<TempSensor>> {
        let stale = self
            .temps_raw
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() >= TEMPS_EVERY);
        if (stale && visible) || self.temps_raw.is_none() {
            let mut fresh = Vec::new();
            if let Some(c) = self.comps.as_mut() {
                c.refresh(false);
                fresh.extend(c.iter().map(|x| (x.label().to_string(), x.temperature())));
            }
            self.temps_raw = Some((Instant::now(), fresh));
        }
        let mut raw: Vec<RawTemp> = self
            .temps_raw
            .as_ref()
            .map(|(_, r)| r.clone())
            .unwrap_or_default();
        raw.extend(
            gpus.iter()
                .filter_map(|g| g.temp_c.map(|t| (format!("GPU {}", g.name), Some(t)))),
        );
        let grouped = temps::group_sensors(&raw);
        if grouped.is_empty() {
            Avail::Unavailable(NO_SENSORS.into())
        } else {
            Avail::Ok(grouped)
        }
    }

    fn procs(&mut self, gpu_usage: &HashMap<u32, f32>) -> Vec<ProcInfo> {
        let kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet)
            .without_tasks();
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        if self.users_at.elapsed().as_secs() >= 60
            && let Some(u) = self.users.as_mut()
        {
            u.refresh();
            self.users_at = Instant::now();
        }
        let users = self.users.as_ref();
        self.sys
            .processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .map(|p| {
                let pid = p.pid().as_u32();
                ProcInfo {
                    pid,
                    name: p.name().to_string_lossy().into_owned(),
                    user: p
                        .user_id()
                        .and_then(|uid| users?.get_user_by_id(uid))
                        .map_or_else(|| "?".to_string(), |u| u.name().to_string()),
                    cpu: p.cpu_usage(),
                    mem: p.memory(),
                    gpu: gpu_usage.get(&pid).copied(),
                    threads: p.tasks().map(|t| t.len()),
                    start_time: p.start_time(),
                    exe: p.exe().map(|e| e.display().to_string()).unwrap_or_default(),
                }
            })
            .collect()
    }
}

/// `/proc/diskstats` names of the mounted devices (`/dev/mapper/data` → `dm-0`), without
/// repeating any.
#[cfg(target_os = "linux")]
fn snap_devices(disks: &Disks) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in disks.iter().filter(|d| d.total_space() > 0) {
        let path = std::path::PathBuf::from(d.name());
        let real = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(name) = real.file_name().map(|n| n.to_string_lossy().into_owned())
            && !out.contains(&name)
        {
            out.push(name);
        }
    }
    out
}

impl Source for SystemSource {
    fn sample(&mut self, ctl: &Control) -> Snapshot {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().max(0.001);
        self.last = now;

        let mut readings: Vec<GpuReading> = self.gpus.iter_mut().flat_map(|g| g.read()).collect();
        readings.sort_by_key(|r| !r.discrete);
        let gpus = if readings.is_empty() {
            Avail::Unavailable("no GPU detected".into())
        } else {
            Avail::Ok(readings.clone())
        };

        let procs = if ctl.procs_visible() {
            let mut usage: HashMap<u32, f32> = HashMap::new();
            for g in self.gpus.iter_mut() {
                for (pid, u) in g.process_usage() {
                    *usage.entry(pid).or_insert(0.0) += u;
                }
            }
            #[cfg(target_os = "linux")]
            {
                let samples = self
                    .drm
                    .scan(std::path::Path::new("/proc"), DRM_RESCAN_EVERY);
                for (pid, u) in self.fdinfo.update(samples, (dt * 1e9) as u64) {
                    *usage.entry(pid).or_insert(0.0) += u;
                }
            }
            Some(self.procs(&usage))
        } else {
            None
        };

        Snapshot {
            taken_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            host: SystemSource::host(),
            cpu: self.cpu(),
            mem: self.mem(),
            temps: self.temps(&readings, !ctl.procs_visible()),
            gpus,
            net: self.net(ctl, dt),
            disk: self.disk(dt),
            procs,
        }
    }
}
