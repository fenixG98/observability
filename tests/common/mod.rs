#![allow(dead_code)]

use observability::app::App;
use observability::config::Config;
use observability::model::*;
use ratatui::{Frame, Terminal, backend::TestBackend};

pub const GIB: u64 = 1 << 30;

pub fn proc_info(pid: u32, name: &str, cpu: f32, mem: u64, gpu: Option<f32>) -> ProcInfo {
    ProcInfo {
        pid,
        name: name.into(),
        user: "alice".into(),
        cpu,
        mem,
        gpu,
        threads: Some(4),
        start_time: 1_000_000 - 3 * 3600 - 12 * 60,
        exe: format!("/usr/bin/{name}"),
    }
}

fn seg(label: &str, gib: f64, kind: MemKind) -> MemSegment {
    MemSegment {
        label: label.into(),
        bytes: (gib * GIB as f64) as u64,
        kind,
    }
}

/// Fixed sample (values from the approved mockup).
pub fn sample_snapshot() -> Snapshot {
    Snapshot {
        taken_at: 1_000_000,
        host: HostInfo {
            hostname: "devhost".into(),
            uptime_secs: 3 * 86400 + 4 * 3600,
            load: [1.42, 0.98, 0.77],
        },
        cpu: CpuSnap {
            model: "Intel(R) Core(TM) Ultra 5 125H".into(),
            total: 34.0,
            per_core: vec![
                42.0, 81.0, 18.0, 65.0, 33.0, 9.0, 71.0, 27.0, 55.0, 12.0, 38.0, 93.0, 22.0, 47.0,
            ],
            freq_mhz: 3100,
        },
        mem: MemSnap {
            total: 16 * GIB,
            segments: vec![
                seg("apps", 6.2, MemKind::Used),
                seg("cache", 4.1, MemKind::Reclaimable),
                seg("buffers", 0.6, MemKind::Reclaimable),
                seg("free", 5.1, MemKind::Free),
            ],
            swap_used: GIB * 8 / 10,
            swap_total: 8 * GIB,
        },
        gpus: Avail::Ok(vec![GpuReading {
            name: "Meteor Lake-P [Intel Graphics]".into(),
            discrete: false,
            util: Avail::Ok(42.0),
            vram_used: None,
            vram_total: None,
            temp_c: None,
            power_w: None,
            engines: vec![("render".into(), 42.0), ("video".into(), 8.0)],
        }]),
        temps: Avail::Ok(vec![
            TempSensor {
                label: "CPU".into(),
                celsius: 68.0,
            },
            TempSensor {
                label: "Cores".into(),
                celsius: 71.0,
            },
            TempSensor {
                label: "Disk".into(),
                celsius: 42.0,
            },
            TempSensor {
                label: "WiFi".into(),
                celsius: 39.0,
            },
        ]),
        net: NetSnap {
            iface_label: "enp0s31f6".into(),
            rx_bps: 12_400_000,
            tx_bps: 1_100_000,
        },
        disk: DiskSnap {
            device: "/dev/mapper/data".into(),
            mounts: vec![
                MountUsage {
                    mount: "/".into(),
                    used: 312 * GIB,
                    total: 472 * GIB,
                },
                MountUsage {
                    mount: "/boot".into(),
                    used: GIB / 10,
                    total: GIB,
                },
            ],
            read_bps: 45_000_000,
            write_bps: 3_000_000,
        },
        procs: Some(vec![
            proc_info(4412, "firefox", 12.8, 1_932_735_283, Some(6.0)),
            proc_info(9921, "rust-analyzer", 8.1, 2_469_606_195, None),
            proc_info(1203, "ghostty", 3.9, 220_200_960, Some(2.0)),
            proc_info(822, "kwin_x11", 3.1, 356_515_840, Some(11.0)),
            proc_info(1, "systemd", 0.2, 14_680_064, None),
        ]),
    }
}

/// App with `n` deterministic samples (values that vary to fill the histories).
pub fn sample_app(n: usize) -> App {
    let mut app = App::new(&Config::default());
    for i in 0..n {
        let mut s = sample_snapshot();
        s.cpu.total = 20.0 + ((i * 37) % 60) as f32;
        if let Avail::Ok(g) = &mut s.gpus {
            g[0].util = Avail::Ok(10.0 + ((i * 23) % 70) as f32);
        }
        s.net.rx_bps = 1_000_000 + ((i * 7_919) % 13) as u64 * 1_000_000;
        s.net.tx_bps = 100_000 + ((i * 104_729) % 7) as u64 * 200_000;
        s.disk.read_bps = ((i * 31) % 9) as u64 * 5_000_000;
        s.disk.write_bps = ((i * 17) % 5) as u64 * 1_000_000;
        app.on_snapshot(s);
    }
    app
}

/// Draws to a test terminal and returns the screen text (one line per row).
pub fn render_text<F: FnOnce(&mut Frame)>(w: u16, h: u16, draw: F) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(draw).unwrap();
    let b = t.backend().buffer();
    (0..b.area.height)
        .map(|y| {
            (0..b.area.width)
                .map(|x| b[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
