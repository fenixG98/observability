use observability::gpu::fdinfo::*;
use std::collections::HashMap;

const I915: &str = "pos:\t0
flags:\t02100002
mnt_id:\t28
ino:\t363
drm-driver:\ti915
drm-client-id:\t5
drm-pdev:\t0000:00:02.0
drm-total-system0:\t0
drm-engine-render:\t1500 ns
drm-engine-copy:\t0 ns
drm-engine-video:\t20 ns
drm-engine-capacity-video:\t2
drm-engine-video-enhance:\t0 ns
drm-engine-compute:\t0 ns
";

#[test]
fn parses_i915_fdinfo() {
    let c = parse_fdinfo(I915).unwrap();
    assert_eq!(c.client_id, "5");
    assert_eq!(c.pdev, "0000:00:02.0");
    assert_eq!(c.engines.len(), 5);
    assert_eq!(c.engines["render"], 1500);
    assert_eq!(c.engines["video"], 20);
    assert!(!c.engines.contains_key("capacity-video"));
    assert!(parse_fdinfo("pos:\t0\nflags:\t0\n").is_none());
}

fn client(id: &str, render: u64, video: u64) -> DrmClient {
    DrmClient {
        client_id: id.into(),
        pdev: "0000:00:02.0".into(),
        engines: HashMap::from([("render".to_string(), render), ("video".to_string(), video)]),
    }
}

#[test]
fn tracker_computes_busiest_engine_percentage() {
    let mut t = FdinfoTracker::default();
    assert!(
        t.update(vec![(10, client("5", 0, 0))], 1_000_000_000)
            .is_empty()
    );
    let u = t.update(
        vec![(10, client("5", 500_000_000, 100_000_000))],
        1_000_000_000,
    );
    assert_eq!(u.get(&10).copied(), Some(50.0));
}

#[test]
fn tracker_dedups_same_client_and_clamps() {
    let mut t = FdinfoTracker::default();
    t.update(
        vec![
            (10, client("5", 0, 0)),
            (10, client("5", 0, 0)),
            (10, client("6", 0, 0)),
        ],
        1,
    );
    let u = t.update(
        vec![
            (10, client("5", 300, 0)),
            (10, client("5", 300, 0)),
            (10, client("6", 5_000, 0)),
        ],
        1_000,
    );
    assert_eq!(u.get(&10).copied(), Some(100.0));
}

#[cfg(unix)]
#[test]
fn scans_only_drm_fds_of_numeric_pids() {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("obs-proc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for pid in ["123", "124", "self"] {
        std::fs::create_dir_all(root.join(pid).join("fd")).unwrap();
        std::fs::create_dir_all(root.join(pid).join("fdinfo")).unwrap();
    }
    symlink("/dev/dri/renderD128", root.join("123/fd/3")).unwrap();
    std::fs::write(root.join("123/fdinfo/3"), I915).unwrap();
    symlink("/dev/null", root.join("124/fd/4")).unwrap();
    std::fs::write(root.join("124/fdinfo/4"), I915).unwrap();
    symlink("/dev/dri/card1", root.join("self/fd/5")).unwrap();
    std::fs::write(root.join("self/fdinfo/5"), I915).unwrap();
    let found = scan_proc(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, 123);
    std::fs::remove_dir_all(&root).unwrap();
}
