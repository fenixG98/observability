mod common;

use common::*;
use guaitop::app::Panel;
use guaitop::keys::Action;
use guaitop::ui::{processes, theme::Theme};

fn theme() -> Theme {
    Theme::new(true, 60.0, 80.0)
}

#[test]
fn processes_tab_shows_table_pie_and_detail() {
    let mut app = sample_app(20);
    app.apply_action(Action::Tab(1));
    app.apply_action(Action::RowDown);
    app.on_snapshot(sample_snapshot());
    let text = render_text(122, 30, |f| processes::render(f, f.area(), &app, &theme()));
    for needle in [
        "PROCESSES",
        "5 · sort: cpu · filter: –",
        "PID",
        "firefox",
        "rust-analyzer",
        "TOP CPU USERS",
        "other",
        "rust-analyzer · 9921",
        "/usr/bin/rust-analyzer",
        "for 3h 12m",
    ] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
    assert!(text.contains("1.8G"), "firefox memory");
    assert!(text.contains("–"), "GPU desconeguda");
    insta::assert_snapshot!(text);
}

#[test]
fn shows_loading_message_before_first_process_sample() {
    let mut app = sample_app(1);
    app.snapshot.procs = None;
    let text = render_text(122, 30, |f| processes::render(f, f.area(), &app, &theme()));
    assert!(text.contains("loading processes"));
}

#[test]
fn zoom_shows_only_the_focused_panel() {
    let mut app = sample_app(3);
    app.apply_action(Action::Tab(1));
    app.focus = Panel::ProcPie;
    app.zoomed = true;
    let text = render_text(122, 30, |f| processes::render(f, f.area(), &app, &theme()));
    assert!(text.contains("TOP CPU USERS"));
    assert!(!text.contains("PROCESSES"));
}

#[test]
fn tiny_terminal_does_not_panic() {
    let app = sample_app(2);
    render_text(10, 4, |f| processes::render(f, f.area(), &app, &theme()));
    render_text(20, 6, |f| processes::render(f, f.area(), &app, &theme()));
    render_text(3, 3, |f| processes::render(f, f.area(), &app, &theme()));
}

#[test]
fn missing_thread_count_shows_dash() {
    let mut app = sample_app(3);
    app.apply_action(Action::Tab(1));
    if let Some(procs) = app.snapshot.procs.as_mut() {
        for p in procs.iter_mut() {
            p.threads = None;
        }
    }
    let text = render_text(122, 30, |f| processes::render(f, f.area(), &app, &theme()));
    assert!(text.contains("threads –"), "{text}");
}
