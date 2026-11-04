mod common;

use common::*;
use observability::model::Avail;
use observability::probe::report;

#[test]
fn report_lists_every_subsystem() {
    let r = report(&sample_snapshot());
    for needle in [
        "cpu: Intel(R) Core(TM) Ultra 5 125H · 14 fils · 34% · 3100 MHz",
        "memòria: 16.0G",
        "apps 6.2G",
        "[1] Meteor Lake-P [Intel Graphics] (integrada) ús 42%",
        "motors: render 42%, video 8%",
        "CPU 68°C",
        "xarxa: enp0s31f6 ↓ 12.4 MB/s ↑ 1.1 MB/s",
        "disc: /dev/mapper/data",
        "/ 312.0G/472.0G",
        "processos: 5 (amb ús de GPU: 3)",
    ] {
        assert!(r.contains(needle), "falta «{needle}»\n{r}");
    }
}

#[test]
fn report_shows_unavailable_reasons() {
    let mut s = sample_snapshot();
    if let Avail::Ok(g) = &mut s.gpus {
        g[0].util = Avail::Unavailable("cal permís: sudo setcap cap_perfmon=ep /x".into());
    }
    s.temps = Avail::Unavailable("no s'ha trobat cap sensor hwmon".into());
    s.procs = None;
    let r = report(&s);
    assert!(
        r.contains("ús NO DISPONIBLE: cal permís: sudo setcap cap_perfmon=ep /x"),
        "{r}"
    );
    assert!(
        r.contains("temperatures: NO DISPONIBLE: no s'ha trobat cap sensor hwmon"),
        "{r}"
    );
    assert!(r.contains("processos: no llegits"), "{r}");
}
