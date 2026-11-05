mod common;

use common::*;
use observability::app::Mode;
use observability::ui::{chrome, theme::Theme};
use ratatui::layout::Rect;

fn theme() -> Theme {
    Theme::new(true, 60.0, 80.0)
}

#[test]
fn tab_bar_shows_tabs_and_host() {
    let app = sample_app(1);
    let text = render_text(122, 1, |f| chrome::tab_bar(f, f.area(), &app, &theme()));
    assert!(text.contains("obstui"));
    assert!(text.contains(" 1 Dashboard "));
    assert!(text.contains(" 2 Processes "));
    assert!(text.contains("devhost · up 3d 4h · load 1.42 0.98 0.77"));
    insta::assert_snapshot!(text);
}

#[test]
fn tab_bar_drops_host_when_narrow_and_shows_prefix() {
    let mut app = sample_app(1);
    app.prefix_armed = true;
    let text = render_text(60, 1, |f| chrome::tab_bar(f, f.area(), &app, &theme()));
    assert!(text.contains("PREFIX"));
    assert!(!text.contains("load"));
}

#[test]
fn status_line_hints_and_messages() {
    let mut app = sample_app(1);
    let text = render_text(122, 1, |f| chrome::status_line(f, f.area(), &app, &theme()));
    assert!(text.contains("hjkl focus"));
    assert!(text.contains("refresh 1000 ms"));
    app.status = Some("sort: mem".into());
    let text = render_text(122, 1, |f| chrome::status_line(f, f.area(), &app, &theme()));
    assert!(text.contains("sort: mem"));
}

#[test]
fn status_line_command_mode_shows_candidates_or_error() {
    let mut app = sample_app(1);
    app.mode = Mode::Command {
        input: "sort c".into(),
        error: None,
    };
    let text = render_text(80, 1, |f| chrome::status_line(f, f.area(), &app, &theme()));
    assert!(text.starts_with(":sort c"));
    assert!(text.contains("cpu"));
    app.mode = Mode::Command {
        input: "foo".into(),
        error: Some("unknown command: foo".into()),
    };
    let text = render_text(80, 1, |f| chrome::status_line(f, f.area(), &app, &theme()));
    assert!(text.contains("unknown command: foo"));
    app.mode = Mode::Filter {
        input: "fire".into(),
    };
    let text = render_text(80, 1, |f| chrome::status_line(f, f.area(), &app, &theme()));
    assert!(text.starts_with("/fire"));
}

#[test]
fn overlays_render_centered() {
    let text = render_text(100, 30, |f| chrome::help_overlay(f, f.area(), &theme()));
    assert!(text.contains("HELP"));
    assert!(text.contains(":sort cpu|mem|gpu|pid|name"));
    insta::assert_snapshot!(text);
    let text = render_text(100, 30, |f| {
        chrome::confirm_overlay(f, f.area(), 4412, "firefox", &theme())
    });
    assert!(text.contains("Send SIGTERM to firefox (4412)?"));
    assert_eq!(
        chrome::centered(Rect::new(0, 0, 100, 30), 50, 5),
        Rect::new(25, 12, 50, 5)
    );
    assert_eq!(
        chrome::centered(Rect::new(0, 0, 20, 4), 50, 5),
        Rect::new(0, 0, 20, 4)
    );
}

#[test]
fn small_areas_do_not_panic() {
    let app = sample_app(1);
    for (w, h) in [(20u16, 6u16), (3, 3), (1, 1)] {
        render_text(w, h, |f| {
            let a = f.area();
            chrome::tab_bar(f, a, &app, &theme());
            chrome::status_line(f, a, &app, &theme());
            chrome::help_overlay(f, a, &theme());
            chrome::confirm_overlay(f, a, 1, "firefox", &theme());
        });
    }
}
