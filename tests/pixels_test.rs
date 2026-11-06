use guaitop::ui::braille::Pie;
use guaitop::ui::pixels::{Graphics, donut_rgba, pie_image_id};
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};
use ratatui_image::picker::{Picker, ProtocolType};

const RED: Color = Color::Rgb(255, 0, 0);
const BLUE: Color = Color::Rgb(0, 0, 255);

fn px(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    img.get_pixel(x, y).0
}

#[test]
fn donut_has_transparent_hole_and_outside() {
    let img = donut_rgba(200, 200, &[(1.0, RED)], 0.6).unwrap();
    assert_eq!(px(&img, 100, 100)[3], 0, "forat");
    assert_eq!(px(&img, 0, 0)[3], 0, "cantonada");
    assert_eq!(px(&img, 100, 10), [255, 0, 0, 255], "anell");
}

#[test]
fn donut_is_a_circle_in_a_non_square_image() {
    // Image taller than wide: the circle fits the width and is centered.
    let img = donut_rgba(100, 160, &[(1.0, RED)], 0.0).unwrap();
    let opaque = |x: u32, y: u32| px(&img, x, y)[3] > 127;
    let rows: Vec<u32> = (0..160).filter(|y| opaque(50, *y)).collect();
    let cols: Vec<u32> = (0..100).filter(|x| opaque(*x, 80)).collect();
    let h = rows.last().unwrap() - rows.first().unwrap() + 1;
    let w = cols.last().unwrap() - cols.first().unwrap() + 1;
    assert!(h.abs_diff(w) <= 1, "width {w} vs height {h}");
    assert!(
        rows.first().unwrap().abs_diff(30) <= 1,
        "centrat verticalment"
    );
}

#[test]
fn donut_edges_are_antialiased() {
    let img = donut_rgba(200, 200, &[(1.0, RED)], 0.0).unwrap();
    let alphas: Vec<u8> = (0..100).map(|x| px(&img, x, 100)[3]).collect();
    assert!(
        alphas.iter().any(|a| *a > 0 && *a < 255),
        "some semi-transparent pixel is needed at the edge: {alphas:?}"
    );
}

#[test]
fn donut_segments_start_at_top_clockwise() {
    let img = donut_rgba(200, 200, &[(0.5, RED), (0.5, BLUE)], 0.0).unwrap();
    assert_eq!(
        px(&img, 150, 100),
        [255, 0, 0, 255],
        "right = first segment"
    );
    assert_eq!(px(&img, 50, 100), [0, 0, 255, 255], "left = second segment");
}

#[test]
fn donut_needs_truecolor_and_data() {
    assert!(donut_rgba(10, 10, &[(1.0, Color::Indexed(4))], 0.0).is_none());
    assert!(donut_rgba(10, 10, &[], 0.0).is_none());
    assert!(donut_rgba(0, 10, &[(1.0, RED)], 0.0).is_none());
}

#[test]
fn image_id_is_stable_per_position() {
    let a = pie_image_id(Rect::new(3, 4, 10, 5));
    assert_eq!(a, pie_image_id(Rect::new(3, 4, 22, 9)));
    assert_ne!(a, pie_image_id(Rect::new(4, 4, 10, 5)));
    assert_ne!(a, 0);
}

fn kitty_graphics() -> Graphics {
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    Graphics::new(picker)
}

#[test]
fn pie_with_graphics_emits_kitty_image_and_keeps_label() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    Pie {
        segments: &[(0.34, RED), (0.66, BLUE)],
        hole: 0.6,
        label: Some(("34%", Color::White)),
        aspect: 2.0,
        graphics: Some(kitty_graphics()),
        cell_px: Some((10, 20)),
    }
    .render(buf.area, &mut buf);
    let all: String = buf.content.iter().map(|c| c.symbol()).collect();
    assert!(all.contains("\x1b_G"), "kitty transmission");
    assert!(all.contains('\u{10EEEE}'), "placeholders unicode");
    let row: String = (0..40).map(|x| buf[(x, 5)].symbol().to_string()).collect();
    assert!(row.contains("34%"), "{row}");
    // The label spans columns 18..21. The next cell must explicitly state again
    // its row and column; otherwise the terminal does not know where the rest of the image row goes.
    let after: Vec<char> = buf[(21, 5)].symbol().chars().collect();
    assert_eq!(after.first(), Some(&'\u{10EEEE}'));
    assert_eq!(after.len(), 4, "placeholder + row + column + id: {after:?}");
    // Same row and id as the first cell of the row; different column.
    let first: Vec<char> = buf[(0, 5)].symbol().chars().collect();
    assert_eq!(after[1], first[1], "row");
    assert_eq!(after[3], first[3], "id");
    assert_ne!(after[2], first[2], "column");
    assert_eq!(buf[(21, 5)].fg, buf[(0, 5)].fg);
}

#[test]
fn pie_without_graphics_uses_text_blocks() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    Pie {
        segments: &[(1.0, RED)],
        hole: 0.0,
        label: None,
        aspect: 2.0,
        graphics: None,
        cell_px: None,
    }
    .render(buf.area, &mut buf);
    let all: String = buf.content.iter().map(|c| c.symbol()).collect();
    assert!(all.contains('█') && !all.contains("\x1b_G"));
}

fn render_pie(g: Graphics, cpu: f64) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    Pie {
        segments: &[(cpu, RED), (1.0 - cpu, BLUE)],
        hole: 0.6,
        label: Some(("34%", Color::White)),
        aspect: 2.0,
        graphics: Some(g),
        cell_px: Some((10, 20)),
    }
    .render(buf.area, &mut buf);
    buf.content.iter().map(|c| c.symbol()).collect()
}

#[test]
fn unchanged_donut_is_not_transmitted_again() {
    let g = kitty_graphics();
    assert!(render_pie(g, 0.34).contains("\x1b_G"), "first transmission");
    let again = render_pie(g, 0.34);
    assert!(
        !again.contains("\x1b_G"),
        "the image is already in the terminal"
    );
    assert!(
        again.contains('\u{10EEEE}'),
        "but the placeholders are there"
    );
    // An invisible change (less than a thousandth) is not retransmitted either.
    assert!(!render_pie(g, 0.3402).contains("\x1b_G"));
    assert!(render_pie(g, 0.40).contains("\x1b_G"), "visible change");
    g.invalidate();
    assert!(render_pie(g, 0.40).contains("\x1b_G"), "after invalidation");
}

#[test]
fn zlib_is_used_on_terminals_known_to_support_it() {
    use guaitop::ui::pixels::kitty_zlib_known;
    assert!(kitty_zlib_known(Some("xterm-ghostty"), None));
    assert!(kitty_zlib_known(Some("xterm-256color"), Some("ghostty")));
    assert!(kitty_zlib_known(Some("xterm-kitty"), None));
    assert!(!kitty_zlib_known(Some("xterm-256color"), Some("WezTerm")));
    assert!(!kitty_zlib_known(None, None));
}
