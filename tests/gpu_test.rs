use guaitop::gpu::sysinfo_gpu::{unavailable_reason, vendor_included};

#[test]
fn vendor_filter() {
    assert!(vendor_included(
        Some("Advanced Micro Devices, Inc."),
        &["nvidia", "intel"]
    ));
    assert!(!vendor_included(Some("NVIDIA Corporation"), &["nvidia"]));
    assert!(!vendor_included(
        Some("Intel Corporation"),
        &["nvidia", "intel"]
    ));
    assert!(vendor_included(None, &["intel"]));
}

#[test]
fn reasons_are_actionable() {
    assert!(unavailable_reason(Some("NVIDIA Corporation")).contains("NVML"));
    assert!(unavailable_reason(Some("Advanced Micro Devices, Inc.")).contains("amdgpu"));
    assert!(unavailable_reason(Some("Acme")).contains("Acme"));
}

#[cfg(target_os = "linux")]
mod intel {
    use guaitop::gpu::GpuProvider;
    use guaitop::gpu::intel::*;
    use guaitop::model::Avail;
    use std::path::Path;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("obs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn parses_event_configs() {
        assert_eq!(parse_event_config("config=0x0\n"), Some(0));
        assert_eq!(parse_event_config("config=0x2010"), Some(0x2010));
        assert_eq!(parse_event_config("event=0x1,config=12"), Some(12));
        assert_eq!(parse_event_config("umask=0x1"), None);
    }

    #[test]
    fn engine_labels() {
        assert_eq!(engine_label("rcs0-busy"), "render");
        assert_eq!(engine_label("vcs1-busy"), "video");
        assert_eq!(engine_label("vecs0-busy"), "video-enh");
        assert_eq!(engine_label("bcs0-busy"), "copy");
        assert_eq!(engine_label("ccs0-busy"), "compute");
        assert_eq!(engine_label("xyz0-busy"), "xyz0");
    }

    #[test]
    fn permission_errors_give_the_setcap_command() {
        let exe = Path::new("/home/u/.cargo/bin/guaitop");
        let e = std::io::Error::from_raw_os_error(libc::EACCES);
        let msg = open_error_reason(&e, exe);
        assert!(
            msg.contains("sudo setcap cap_perfmon=ep /home/u/.cargo/bin/guaitop"),
            "{msg}"
        );
        assert!(msg.contains("rebuild"));
        let e = std::io::Error::from_raw_os_error(libc::ENODEV);
        assert!(open_error_reason(&e, exe).contains("PMU"));
    }

    #[test]
    fn finds_pmu_and_busy_events() {
        let root = tmp("pmu");
        let pmu = root.join("i915");
        std::fs::create_dir_all(pmu.join("events")).unwrap();
        std::fs::write(pmu.join("type"), "14\n").unwrap();
        std::fs::write(pmu.join("events/rcs0-busy"), "config=0x0\n").unwrap();
        std::fs::write(pmu.join("events/vcs0-busy"), "config=0x2000\n").unwrap();
        std::fs::write(pmu.join("events/rcs0-wait"), "config=0x1\n").unwrap();
        std::fs::create_dir_all(root.join("cpu")).unwrap();
        assert_eq!(find_pmu(&root), Some(pmu.clone()));
        let mut ev = list_busy_events(&pmu);
        ev.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            ev,
            vec![
                PmuEvent {
                    name: "rcs0-busy".into(),
                    config: 0
                },
                PmuEvent {
                    name: "vcs0-busy".into(),
                    config: 0x2000
                },
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn finds_intel_card_and_driver() {
        use std::os::unix::fs::symlink;
        let drm = tmp("drm");
        std::fs::create_dir_all(drm.join("card0/device")).unwrap();
        std::fs::write(drm.join("card0/device/vendor"), "0x10de\n").unwrap();
        std::fs::create_dir_all(drm.join("card1/device")).unwrap();
        std::fs::write(drm.join("card1/device/vendor"), "0x8086\n").unwrap();
        symlink(
            "../../../bus/pci/drivers/i915",
            drm.join("card1/device/driver"),
        )
        .unwrap();
        std::fs::create_dir_all(drm.join("card1-eDP-1")).unwrap();
        let card = find_intel_card(&drm).unwrap();
        assert_eq!(
            card,
            IntelCard {
                card: "card1".into(),
                driver: "i915".into()
            }
        );
        std::fs::remove_dir_all(&drm).unwrap();
    }

    #[test]
    fn failed_provider_reports_reason() {
        let mut g = IntelGpu::failed("Meteor Lake-P [Intel Graphics]", "permission needed");
        let r = g.read();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].util, Avail::Unavailable("permission needed".into()));
        assert!(!r[0].discrete);
        assert_eq!(r[0].vram_total, None);
    }
}
