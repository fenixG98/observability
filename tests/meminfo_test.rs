use guaitop::collect::meminfo::*;
use guaitop::model::MemKind;

const MEMINFO: &str = "MemTotal:       16000000 kB
MemFree:         4000000 kB
MemAvailable:    9000000 kB
Buffers:          500000 kB
Cached:          4000000 kB
SwapCached:            0 kB
Shmem:            300000 kB
SReclaimable:     300000 kB
SwapTotal:       8000000 kB
SwapFree:        7000000 kB
";

#[test]
fn parses_linux_meminfo_like_htop() {
    let m = parse_linux_meminfo(MEMINFO).unwrap();
    let kib = 1024u64;
    assert_eq!(m.total, 16_000_000 * kib);
    let by = |l: &str| m.segments.iter().find(|s| s.label == l).unwrap().clone();
    assert_eq!(by("apps").bytes, 7_500_000 * kib);
    assert_eq!(by("apps").kind, MemKind::Used);
    assert_eq!(by("cache").bytes, 4_000_000 * kib);
    assert_eq!(by("cache").kind, MemKind::Reclaimable);
    assert_eq!(by("buffers").bytes, 500_000 * kib);
    assert_eq!(by("free").bytes, 4_000_000 * kib);
    assert_eq!(by("free").kind, MemKind::Free);
    assert_eq!(m.swap_used, 1_000_000 * kib);
    assert_eq!(m.swap_total, 8_000_000 * kib);
    assert!((m.used_fraction() - 0.46875).abs() < 1e-9);
}

#[test]
fn missing_total_is_an_error() {
    assert!(
        parse_linux_meminfo("MemFree: 1 kB\n")
            .unwrap_err()
            .contains("MemTotal")
    );
}

#[test]
fn macos_segments_add_up_to_total() {
    let v = MacVm {
        wired: 10,
        compressed: 5,
        purgeable: 5,
        external: 15,
        internal: 40,
    };
    let m = macos_segments(&v, 100, 3, 8);
    let labels: Vec<_> = m
        .segments
        .iter()
        .map(|s| (s.label.as_str(), s.bytes))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("apps", 35),
            ("wired", 10),
            ("compressed", 5),
            ("cache", 20),
            ("free", 30)
        ]
    );
    assert_eq!(m.segments.iter().map(|s| s.bytes).sum::<u64>(), 100);
    assert!((m.used_fraction() - 0.5).abs() < 1e-9);
    assert_eq!((m.swap_used, m.swap_total), (3, 8));
}

#[test]
fn huge_linux_values_do_not_panic() {
    let text = "MemTotal: 18446744073709551615 kB\nMemFree: 18446744073709551615 kB\nBuffers: 18446744073709551615 kB\nCached: 18446744073709551615 kB\nSReclaimable: 18446744073709551615 kB\n";
    let _ = parse_linux_meminfo(text).unwrap();
}

#[test]
fn huge_macos_values_do_not_panic() {
    let m = u64::MAX;
    let v = MacVm {
        wired: m,
        compressed: m,
        purgeable: m,
        external: m,
        internal: m,
    };
    let _ = macos_segments(&v, m, 0, 0);
}
