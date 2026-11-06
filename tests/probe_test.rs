mod common;

use common::*;
use guaitop::model::Avail;
use guaitop::probe::report;

#[test]
fn report_lists_every_subsystem() {
    let r = report(&sample_snapshot());
    for needle in [
        "cpu: Intel(R) Core(TM) Ultra 5 125H · 14 threads · 34% · 3100 MHz",
        "memory: 16.0G",
        "apps 6.2G",
        "[1] Meteor Lake-P [Intel Graphics] (integrated) usage 42%",
        "engines: render 42%, video 8%",
        "CPU 68°C",
        "network: enp0s31f6 ↓ 12.4 MB/s ↑ 1.1 MB/s",
        "disk: /dev/mapper/data",
        "/ 312.0G/472.0G",
        "processes: 5 (using the GPU: 3)",
    ] {
        assert!(r.contains(needle), "falta «{needle}»\n{r}");
    }
}

#[test]
fn report_shows_unavailable_reasons() {
    let mut s = sample_snapshot();
    if let Avail::Ok(g) = &mut s.gpus {
        g[0].util = Avail::Unavailable("permission needed: sudo setcap cap_perfmon=ep /x".into());
    }
    s.temps = Avail::Unavailable("no hwmon sensor found".into());
    s.procs = None;
    let r = report(&s);
    assert!(
        r.contains("usage NOT AVAILABLE: permission needed: sudo setcap cap_perfmon=ep /x"),
        "{r}"
    );
    assert!(
        r.contains("temperatures: NOT AVAILABLE: no hwmon sensor found"),
        "{r}"
    );
    assert!(r.contains("processes: not read"), "{r}");
}
