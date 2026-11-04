//! Text report for `--probe`: what the tool detects on this machine, without the TUI.

use std::fmt::Write;

use crate::model::{Avail, Snapshot, human_bytes, human_duration, human_rate};

pub fn report(s: &Snapshot) -> String {
    let mut o = String::new();
    let _ = writeln!(
        o,
        "observability {} — diagnòstic",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(
        o,
        "host: {} (en marxa {})",
        s.host.hostname,
        human_duration(s.host.uptime_secs)
    );
    let _ = writeln!(
        o,
        "cpu: {} · {} fils · {:.0}% · {} MHz",
        s.cpu.model,
        s.cpu.per_core.len(),
        s.cpu.total,
        s.cpu.freq_mhz
    );
    let segs: Vec<String> = s
        .mem
        .segments
        .iter()
        .map(|m| format!("{} {}", m.label, human_bytes(m.bytes)))
        .collect();
    let _ = writeln!(
        o,
        "memòria: {} · {} · swap {}/{}",
        human_bytes(s.mem.total),
        segs.join(", "),
        human_bytes(s.mem.swap_used),
        human_bytes(s.mem.swap_total)
    );
    match &s.gpus {
        Avail::Unavailable(r) => {
            let _ = writeln!(o, "gpu: NO DISPONIBLE: {r}");
        }
        Avail::Ok(list) => {
            let _ = writeln!(o, "gpu:");
            for (i, g) in list.iter().enumerate() {
                let kind = if g.discrete { "dedicada" } else { "integrada" };
                let util = match &g.util {
                    Avail::Ok(u) => format!("ús {u:.0}%"),
                    Avail::Unavailable(r) => format!("ús NO DISPONIBLE: {r}"),
                };
                let mut extra = Vec::new();
                if let (Some(u), Some(t)) = (g.vram_used, g.vram_total) {
                    extra.push(format!("VRAM {}/{}", human_bytes(u), human_bytes(t)));
                }
                if let Some(t) = g.temp_c {
                    extra.push(format!("{t:.0}°C"));
                }
                if let Some(p) = g.power_w {
                    extra.push(format!("{p:.0} W"));
                }
                let extra = if extra.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", extra.join(" · "))
                };
                let _ = writeln!(o, "  [{}] {} ({kind}) {util}{extra}", i + 1, g.name);
                if !g.engines.is_empty() {
                    let engines: Vec<String> = g
                        .engines
                        .iter()
                        .map(|(e, v)| format!("{e} {v:.0}%"))
                        .collect();
                    let _ = writeln!(o, "      motors: {}", engines.join(", "));
                }
            }
        }
    }
    match &s.temps {
        Avail::Unavailable(r) => {
            let _ = writeln!(o, "temperatures: NO DISPONIBLE: {r}");
        }
        Avail::Ok(list) => {
            let items: Vec<String> = list
                .iter()
                .map(|t| format!("{} {:.0}°C", t.label, t.celsius))
                .collect();
            let _ = writeln!(o, "temperatures: {}", items.join(", "));
        }
    }
    let _ = writeln!(
        o,
        "xarxa: {} ↓ {} ↑ {}",
        s.net.iface_label,
        human_rate(s.net.rx_bps),
        human_rate(s.net.tx_bps)
    );
    let mounts: Vec<String> = s
        .disk
        .mounts
        .iter()
        .map(|m| {
            format!(
                "{} {}/{}",
                m.mount,
                human_bytes(m.used),
                human_bytes(m.total)
            )
        })
        .collect();
    let _ = writeln!(
        o,
        "disc: {} · {} · llegir {} · escriure {}",
        s.disk.device,
        mounts.join(", "),
        human_rate(s.disk.read_bps),
        human_rate(s.disk.write_bps)
    );
    match &s.procs {
        None => {
            let _ = writeln!(o, "processos: no llegits");
        }
        Some(p) => {
            let with_gpu = p.iter().filter(|x| x.gpu.is_some()).count();
            let _ = writeln!(o, "processos: {} (amb ús de GPU: {with_gpu})", p.len());
        }
    }
    o
}
