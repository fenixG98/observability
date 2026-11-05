use observability::collect::system::SystemSource;
use observability::collect::{Control, Source, kill_process, spawn};
use observability::model::Snapshot;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

struct Fake {
    n: u64,
}

impl Source for Fake {
    fn sample(&mut self, ctl: &Control) -> Snapshot {
        self.n += 1;
        Snapshot {
            taken_at: self.n,
            procs: ctl.procs_visible().then(Vec::new),
            ..Default::default()
        }
    }
}

#[test]
fn control_setters() {
    let ctl = Control::new(Duration::from_millis(1000), Some("eth0".into()));
    assert_eq!(ctl.interval(), Duration::from_millis(1000));
    ctl.set_interval(Duration::from_millis(250));
    assert_eq!(ctl.interval(), Duration::from_millis(250));
    assert_eq!(ctl.net_iface().as_deref(), Some("eth0"));
    ctl.set_net_iface(None);
    assert_eq!(ctl.net_iface(), None);
    assert!(!ctl.procs_visible());
    ctl.set_procs_visible(true);
    assert!(ctl.procs_visible());
}

#[test]
fn collector_publishes_and_obeys_control() {
    let ctl = Control::new(Duration::from_millis(100), None);
    let rx = spawn(ctl.clone(), || Fake { n: 0 });
    let first = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(first.taken_at, 1);
    assert!(first.procs.is_none());
    ctl.set_procs_visible(true);
    let with_procs = loop {
        let s = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if s.procs.is_some() {
            break s;
        }
    };
    assert!(with_procs.taken_at >= 2);
    ctl.stop();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Err(RecvTimeoutError::Disconnected) => break,
            _ => assert!(
                Instant::now() < deadline,
                "the collector thread did not stop"
            ),
        }
    }
}

#[test]
fn kill_rejects_invalid_pids() {
    assert!(kill_process(0).is_err());
    assert!(kill_process(1).is_err());
    assert!(kill_process(std::process::id()).is_err());
    assert!(kill_process(u32::MAX).is_err());
    assert!(kill_process(999_999_999).is_err());
}

#[test]
fn system_source_smoke() {
    let ctl = Control::new(Duration::from_millis(500), None);
    let mut src = SystemSource::new(Vec::new());
    let _ = src.sample(&ctl);
    std::thread::sleep(Duration::from_millis(300));
    let s = src.sample(&ctl);
    assert!(s.taken_at > 1_600_000_000);
    assert!(!s.cpu.per_core.is_empty());
    assert!(s.mem.total > 0);
    assert!(!s.mem.segments.is_empty());
    assert!(!s.disk.mounts.is_empty());
    assert!(s.procs.is_none());
    assert_eq!(s.gpus.reason(), Some("no GPU detected"));
    ctl.set_procs_visible(true);
    let s = src.sample(&ctl);
    let me = std::process::id();
    assert!(s.procs.unwrap().iter().any(|p| p.pid == me));
}

#[test]
fn collector_wakes_at_once_on_stop_and_interval_change() {
    // Long interval: the thread does not wake on its own, but must react immediately to changes.
    let ctl = Control::new(Duration::from_secs(30), None);
    let rx = spawn(ctl.clone(), || Fake { n: 0 });
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let t = Instant::now();
    ctl.set_interval(Duration::from_millis(20));
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "{:?}",
        t.elapsed()
    );
    ctl.set_interval(Duration::from_secs(30));
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    ctl.stop();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(2)),
        Err(RecvTimeoutError::Disconnected)
    ));
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "{:?}",
        t.elapsed()
    );
}
