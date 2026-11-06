use guaitop::ui::theme::*;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

#[test]
fn detects_truecolor_from_colorterm() {
    assert!(detect_truecolor(Some("truecolor")));
    assert!(detect_truecolor(Some("24BIT")));
    assert!(!detect_truecolor(Some("256")));
    assert!(!detect_truecolor(None));
}

#[test]
fn palette_depends_on_truecolor() {
    assert!(matches!(Theme::new(true, 60.0, 80.0).fg, Color::Rgb(..)));
    assert!(matches!(
        Theme::new(false, 60.0, 80.0).fg,
        Color::Indexed(_)
    ));
}

#[test]
fn level_and_temp_thresholds() {
    let t = Theme::new(true, 60.0, 80.0);
    assert_eq!(t.level(0.2), t.ok);
    assert_eq!(t.level(0.5), t.warm);
    assert_eq!(t.level(0.8), t.hot);
    assert_eq!(t.temp(59.9), t.ok);
    assert_eq!(t.temp(60.0), t.warm);
    assert_eq!(t.temp(85.0), t.hot);
}

#[test]
fn panel_renders_title_right_text_and_focus() {
    let t = Theme::new(true, 60.0, 80.0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    t.panel("CPU", "34%", true).render(buf.area, &mut buf);
    let top: String = (0..30).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert!(top.starts_with("╭ CPU "), "{top}");
    assert!(top.contains(" 34% "), "{top}");
    assert_eq!(buf[(0, 0)].fg, t.focus);
}

#[test]
fn truncate_adds_ellipsis() {
    assert_eq!(truncate("firefox", 10), "firefox");
    assert_eq!(truncate("rust-analyzer", 6), "rust-…");
    assert_eq!(truncate("àèò", 2), "à…");
}
