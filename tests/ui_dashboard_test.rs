mod common;

use common::*;
use observability::app::Panel;
use observability::model::Avail;
use observability::ui::{compact, dashboard, theme::Theme};

fn theme() -> Theme {
    Theme::new(true, 60.0, 80.0)
}

#[test]
fn dashboard_layout_b_shows_every_panel() {
    let app = sample_app(60);
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    for needle in [
        "CPU",
        "MEMÒRIA",
        "GPU",
        "DISC /",
        "HISTORIAL CPU · GPU",
        "TEMPERATURES",
        "XARXA  enp0s31f6",
        "DISC I/O",
    ] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
    assert!(text.contains("Core Ultra 5 125H"));
    assert!(text.contains("14 fils · 3.1 GHz"));
    assert!(text.contains("66%"), "disk 312/472");
    assert!(text.contains("68°C"));
    assert!(text.contains("↓ ") && text.contains("↑ "), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn gpu_panel_explains_missing_permission() {
    let mut app = sample_app(2);
    if let Avail::Ok(g) = &mut app.snapshot.gpus {
        g[0].util = Avail::Unavailable(
            "cal permís per llegir la GPU Intel: sudo setcap cap_perfmon=ep /usr/bin/observability"
                .into(),
        );
    }
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("sudo setcap"), "{text}");
    assert!(text.contains("cap_perfmon=ep"), "{text}");
}

#[test]
fn unavailable_temps_and_no_gpu_show_reasons() {
    let mut app = sample_app(2);
    app.snapshot.temps = Avail::Unavailable("no s'ha trobat cap sensor hwmon".into());
    app.snapshot.gpus = Avail::Unavailable("no s'ha detectat cap GPU".into());
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("no s'ha trobat cap sensor hwmon"));
    assert!(text.contains("no s'ha detectat cap GPU"));
}

#[test]
fn zoom_renders_only_the_focused_panel_with_legend() {
    let mut app = sample_app(10);
    app.focus = Panel::MemDonut;
    app.zoomed = true;
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("MEMÒRIA"));
    assert!(!text.contains("TEMPERATURES"));
    for label in ["apps", "cache", "buffers", "lliure"] {
        assert!(text.contains(label), "missing legend {label}");
    }
}

#[test]
fn short_model_strips_marketing() {
    assert_eq!(
        dashboard::short_model("Intel(R) Core(TM) Ultra 5 125H"),
        "Core Ultra 5 125H"
    );
    assert_eq!(dashboard::short_model("Apple M3 Pro"), "Apple M3 Pro");
    assert_eq!(
        dashboard::short_model("AMD Ryzen 9 7950X 16-Core Processor"),
        "AMD Ryzen 9 7950X 16-Core"
    );
}

#[test]
fn compact_view_fits_80x24() {
    let app = sample_app(30);
    let text = render_text(80, 22, |f| compact::render(f, f.area(), &app, &theme()));
    for needle in ["RESUM", "CPU", "GPU", "MEM", "SWAP", "DISC", "↓", "↑"] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
    insta::assert_snapshot!(text);
}

#[test]
fn tiny_terminals_do_not_panic() {
    let app = sample_app(5);
    render_text(20, 6, |f| dashboard::render(f, f.area(), &app, &theme()));
    render_text(20, 6, |f| compact::render(f, f.area(), &app, &theme()));
    render_text(3, 3, |f| dashboard::render(f, f.area(), &app, &theme()));
    render_text(10, 4, |f| dashboard::render(f, f.area(), &app, &theme()));
}
