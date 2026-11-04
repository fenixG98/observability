//! Memory breakdown per platform.

use std::collections::HashMap;

use crate::model::{MemKind, MemSegment, MemSnap};

fn seg(label: &str, bytes: u64, kind: MemKind) -> MemSegment {
    MemSegment {
        label: label.into(),
        bytes,
        kind,
    }
}

/// Parses `/proc/meminfo` (values in kB) with the same criteria as htop:
/// cache = Cached + SReclaimable − Shmem; apps = total − free − buffers − cache.
pub fn parse_linux_meminfo(text: &str) -> Result<MemSnap, String> {
    let mut map: HashMap<&str, u64> = HashMap::new();
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        if let Some(n) = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u64>().ok())
        {
            map.insert(key.trim(), n.saturating_mul(1024));
        }
    }
    let get = |k: &str| map.get(k).copied();
    let total = get("MemTotal").ok_or("falta MemTotal a /proc/meminfo")?;
    let free = get("MemFree").ok_or("falta MemFree a /proc/meminfo")?;
    let buffers = get("Buffers").unwrap_or(0);
    let cache = get("Cached")
        .unwrap_or(0)
        .saturating_add(get("SReclaimable").unwrap_or(0))
        .saturating_sub(get("Shmem").unwrap_or(0));
    let apps = total.saturating_sub(free.saturating_add(buffers).saturating_add(cache));
    let swap_total = get("SwapTotal").unwrap_or(0);
    let swap_free = get("SwapFree").unwrap_or(0);
    Ok(MemSnap {
        total,
        segments: vec![
            seg("apps", apps, MemKind::Used),
            seg("cache", cache, MemKind::Reclaimable),
            seg("buffers", buffers, MemKind::Reclaimable),
            seg("lliure", free, MemKind::Free),
        ],
        swap_used: swap_total.saturating_sub(swap_free),
        swap_total,
    })
}

/// macOS `host_statistics64` counters, already converted to bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacVm {
    pub wired: u64,
    pub compressed: u64,
    pub purgeable: u64,
    pub external: u64,
    pub internal: u64,
}

/// Segments in the style of Activity Monitor. "free" is the remainder up to the total.
pub fn macos_segments(v: &MacVm, total: u64, swap_used: u64, swap_total: u64) -> MemSnap {
    let apps = v.internal.saturating_sub(v.purgeable);
    let cache = v.external.saturating_add(v.purgeable);
    let used = apps.saturating_add(v.wired).saturating_add(v.compressed);
    let free = total.saturating_sub(used.saturating_add(cache));
    MemSnap {
        total,
        segments: vec![
            seg("apps", apps, MemKind::Used),
            seg("wired", v.wired, MemKind::Used),
            seg("comprimida", v.compressed, MemKind::Used),
            seg("cache", cache, MemKind::Reclaimable),
            seg("lliure", free, MemKind::Free),
        ],
        swap_used,
        swap_total,
    }
}

/// Reads macOS virtual memory statistics.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn read_macos(total: u64, swap_used: u64, swap_total: u64) -> Result<MemSnap, String> {
    // SAFETY: FFI call with a zero-initialized struct of the size the kernel expects.
    unsafe {
        let mut stats: libc::vm_statistics64 = std::mem::zeroed();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        let ret = libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            &mut stats as *mut libc::vm_statistics64 as libc::host_info64_t,
            &mut count,
        );
        if ret != libc::KERN_SUCCESS {
            return Err(format!("host_statistics64 ha fallat ({ret})"));
        }
        let page = libc::sysconf(libc::_SC_PAGESIZE) as u64;
        let v = MacVm {
            wired: stats.wire_count as u64 * page,
            compressed: stats.compressor_page_count as u64 * page,
            purgeable: stats.purgeable_count as u64 * page,
            external: stats.external_page_count as u64 * page,
            internal: stats.internal_page_count as u64 * page,
        };
        Ok(macos_segments(&v, total, swap_used, swap_total))
    }
}
