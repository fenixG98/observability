//! Real data source: sysinfo + GPU providers + platform parsers.

use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{
    Components, Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users,
};

use crate::collect::{Control, Source, netfilter, temps};
use crate::gpu::GpuProvider;
use crate::gpu::fdinfo::FdinfoTracker;
use crate::model::*;

#[cfg(target_os = "macos")]
const NO_SENSORS: &str = "no s'han pogut llegir els sensors (IOHID/SMC)";
#[cfg(not(target_os = "macos"))]
const NO_SENSORS: &str =
    "no s'ha trobat cap sensor hwmon (carrega el mòdul del sensor, p. ex. coretemp o k10temp)";

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
    last: Instant,
}

impl SystemSource {
    pub fn new(gpus: Vec<Box<dyn GpuProvider>>) -> SystemSource {
        let mut sys = System::new().expect("sysinfo does not support this operating system");
        sys.refresh_cpu_all();
        SystemSource {
            sys,
            nets: Networks::new_with_refreshed_list().ok(),
            disks: Disks::new_with_refreshed_list().ok(),
            comps: Components::new_with_refreshed_list().ok(),
            users: Users::new_with_refreshed_list().ok(),
            users_at: Instant::now(),
            gpus,
            fdinfo: FdinfoTracker::default(),
            last: Instant::now(),
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
                    label: "usada".into(),
                    bytes: used,
                    kind: MemKind::Used,
                },
                MemSegment {
                    label: "lliure".into(),
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
        let Some(nets) = self.nets.as_mut() else {
            return NetSnap {
                iface_label: "xarxa no disponible".into(),
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

    fn disk(&mut self, dt: f64) -> DiskSnap {
        let Some(disks) = self.disks.as_mut() else {
            return DiskSnap::default();
        };
        disks.refresh(true);
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
        DiskSnap {
            device,
            mounts,
            read_bps: (read as f64 / dt) as u64,
            write_bps: (written as f64 / dt) as u64,
        }
    }

    fn temps(&mut self, gpus: &[GpuReading]) -> Avail<Vec<TempSensor>> {
        let mut raw: Vec<(String, Option<f32>)> = Vec::new();
        if let Some(c) = self.comps.as_mut() {
            c.refresh(false);
            raw.extend(c.iter().map(|x| (x.label().to_string(), x.temperature())));
        }
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

impl Source for SystemSource {
    fn sample(&mut self, ctl: &Control) -> Snapshot {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().max(0.001);
        self.last = now;

        let mut readings: Vec<GpuReading> = self.gpus.iter_mut().flat_map(|g| g.read()).collect();
        readings.sort_by_key(|r| !r.discrete);
        let gpus = if readings.is_empty() {
            Avail::Unavailable("no s'ha detectat cap GPU".into())
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
                let samples = crate::gpu::fdinfo::scan_proc(std::path::Path::new("/proc"));
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
            temps: self.temps(&readings),
            gpus,
            net: self.net(ctl, dt),
            disk: self.disk(dt),
            procs,
        }
    }
}
