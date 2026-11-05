mod common;

use common::*;
use observability::app::Mode;
use observability::keys::Action;
use observability::ui::{self, theme::Theme};

fn theme() -> Theme {
    Theme::new(true, 60.0, 80.0)
}

#[test]
fn full_screen_dashboard() {
    let app = sample_app(60);
    let text = render_text(122, 38, |f| ui::draw(f, &app, &theme()));
    assert!(text.lines().next().unwrap().contains("obstui"));
    assert!(text.contains("HISTORY CPU · GPU"));
    assert!(text.lines().last().unwrap().contains("hjkl focus"));
}

#[test]
fn small_terminal_uses_compact_view() {
    let app = sample_app(10);
    let text = render_text(90, 24, |f| ui::draw(f, &app, &theme()));
    assert!(text.contains("SUMMARY"));
    assert!(!text.contains("HISTORY"));
}

#[test]
fn processes_tab_and_overlays() {
    let mut app = sample_app(5);
    app.apply_action(Action::Tab(1));
    let text = render_text(122, 38, |f| ui::draw(f, &app, &theme()));
    assert!(text.contains("PROCESSES"));
    app.mode = Mode::Help;
    assert!(render_text(122, 38, |f| ui::draw(f, &app, &theme())).contains("HELP"));
    app.mode = Mode::ConfirmKill {
        pid: 4412,
        name: "firefox".into(),
    };
    assert!(
        render_text(122, 38, |f| ui::draw(f, &app, &theme())).contains("Send SIGTERM to firefox")
    );
    app.mode = Mode::Command {
        input: "proc".into(),
        error: None,
    };
    let text = render_text(122, 38, |f| ui::draw(f, &app, &theme()));
    assert!(text.lines().last().unwrap().starts_with(":proc"));
}
