use observability::model::*;

#[test]
fn avail_accessors() {
    let ok: Avail<u32> = Avail::Ok(3);
    assert_eq!(ok.ok(), Some(&3));
    assert_eq!(ok.reason(), None);
    let no: Avail<u32> = Avail::Unavailable("cal permís".into());
    assert_eq!(no.ok(), None);
    assert_eq!(no.reason(), Some("cal permís"));
    let def: Avail<u32> = Avail::default();
    assert_eq!(def.reason(), Some("sense dades encara"));
}

#[test]
fn used_fraction_counts_only_used_segments() {
    let m = MemSnap {
        total: 100,
        segments: vec![
            MemSegment {
                label: "apps".into(),
                bytes: 40,
                kind: MemKind::Used,
            },
            MemSegment {
                label: "cache".into(),
                bytes: 30,
                kind: MemKind::Reclaimable,
            },
            MemSegment {
                label: "lliure".into(),
                bytes: 30,
                kind: MemKind::Free,
            },
        ],
        swap_used: 0,
        swap_total: 0,
    };
    assert!((m.used_fraction() - 0.40).abs() < 1e-9);
    assert_eq!(MemSnap::default().used_fraction(), 0.0);
}

#[test]
fn human_bytes_uses_binary_units() {
    assert_eq!(human_bytes(0), "0B");
    assert_eq!(human_bytes(512), "512B");
    assert_eq!(human_bytes(2048), "2K");
    assert_eq!(human_bytes(340 * 1024 * 1024), "340M");
    assert_eq!(human_bytes(1_932_735_283), "1.8G");
}

#[test]
fn human_rate_uses_decimal_units() {
    assert_eq!(human_rate(500), "500 B/s");
    assert_eq!(human_rate(1_100), "1.1 KB/s");
    assert_eq!(human_rate(12_400_000), "12.4 MB/s");
    assert_eq!(human_rate(2_500_000_000), "2.5 GB/s");
}

#[test]
fn human_duration_picks_two_units() {
    assert_eq!(human_duration(42), "42s");
    assert_eq!(human_duration(12 * 60 + 5), "12m");
    assert_eq!(human_duration(4 * 3600 + 12 * 60), "4h 12m");
    assert_eq!(human_duration(3 * 86400 + 4 * 3600 + 59), "3d 4h");
}
