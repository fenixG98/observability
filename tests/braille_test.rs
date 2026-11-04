use observability::ui::braille::*;
use observability::ui::theme::{Theme, cell_aspect};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
    widgets::Widget,
};

/// Quadrant bits (UL=1, UR=2, LL=4, LR=8) of a character, or `None` if it is not one.
fn quadrant_bits(ch: char) -> Option<u8> {
    (0u8..16).find(|b| quadrant(*b) == ch)
}

fn is_braille(ch: char) -> bool {
    ('\u{2800}'..='\u{28FF}').contains(&ch)
}

/// Physical extent (width, height) in cell-width units of the filled subpixels.
fn physical_extent(cells: &CellGrid<PieCell>, aspect: f64) -> (f64, f64) {
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for (r, row) in cells.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let Some((ch, pc)) = cell else { continue };
            let bits = quadrant_bits(*ch).expect("quadrant character");
            for (i, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].iter().enumerate() {
                // A subpixel counts as filled if fg or bg paints it (not the terminal background).
                let on = bits & (1 << i) != 0 || pc.bg.is_some();
                if on {
                    let (x, y) = (c * 2 + dx, r * 2 + dy);
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
    }
    (
        (x1 - x0 + 1) as f64 / 2.0,
        (y1 - y0 + 1) as f64 / 2.0 * aspect,
    )
}

#[test]
fn quadrant_table_is_complete() {
    assert_eq!(quadrant(0), ' ');
    assert_eq!(quadrant(0b0011), '▀');
    assert_eq!(quadrant(0b1100), '▄');
    assert_eq!(quadrant(0b0101), '▌');
    assert_eq!(quadrant(0b1010), '▐');
    assert_eq!(quadrant(0b1111), '█');
    let all: std::collections::HashSet<char> = (0u8..16).map(quadrant).collect();
    assert_eq!(all.len(), 16);
}

#[test]
fn cell_aspect_from_pixels() {
    // 100×30 cells in 1000×660 px → 10×22 px cell.
    assert!((cell_aspect(100, 30, 1000, 660) - 2.2).abs() < 1e-9);
    // Terminal that reports no pixels → classic 1:2 ratio.
    assert_eq!(cell_aspect(100, 30, 0, 0), 2.0);
    assert_eq!(cell_aspect(0, 0, 1000, 660), 2.0);
    // Absurd values are clamped.
    assert_eq!(cell_aspect(1, 1, 100, 1), 1.0);
    assert_eq!(cell_aspect(1, 1, 1, 100), 4.0);
    assert_eq!(Theme::new(true, 60.0, 80.0).cell_aspect, 2.0);
}

#[test]
fn pie_without_data_is_empty() {
    let cells = pie_cells(8, 4, &[], 0.0, 2.0);
    assert!(cells.iter().flatten().all(|c| c.is_none()));
    let cells = pie_cells(8, 4, &[(0.0, Color::Red)], 0.0, 2.0);
    assert!(cells.iter().flatten().all(|c| c.is_none()));
}

#[test]
fn pie_is_a_true_circle_for_any_cell_aspect() {
    for aspect in [2.0, 2.2, 2.5, 1.8] {
        let (w, h) = pie_size(200, 20, aspect);
        let cells = pie_cells(w, h, &[(1.0, Color::Red)], 0.0, aspect);
        let (pw, ph) = physical_extent(&cells, aspect);
        // Tolerance: one subpixel on each axis.
        let tol = 0.5 + aspect / 2.0;
        assert!(
            (pw - ph).abs() <= tol,
            "aspect {aspect}: width {pw} vs height {ph} (grid {w}×{h})"
        );
    }
}

#[test]
fn pie_size_fits_area_and_keeps_proportion() {
    assert_eq!(pie_size(200, 10, 2.2), (22, 10));
    // Narrow area: the width rules.
    assert_eq!(pie_size(11, 10, 2.2), (11, 5));
    assert_eq!(pie_size(1, 10, 2.0), (1, 1));
}

#[test]
fn full_pie_is_opaque_and_hole_empties_center() {
    let cells = pie_cells(16, 8, &[(1.0, Color::Red)], 0.0, 2.0);
    assert!(
        cells
            .iter()
            .flatten()
            .flatten()
            .all(|(ch, _)| !is_braille(*ch))
    );
    assert!(
        cells
            .iter()
            .flatten()
            .flatten()
            .all(|(_, pc)| pc.fg == Color::Red),
        "a single color"
    );
    // The interior is completely filled.
    for (r, c) in [(3, 7), (4, 8), (2, 6), (5, 9)] {
        assert_eq!(cells[r][c].map(|x| x.0), Some('█'), "cell {r},{c}");
    }
    let donut = pie_cells(16, 8, &[(1.0, Color::Red)], 0.6, 2.0);
    assert!(donut[3][7].is_none() && donut[4][8].is_none());
}

#[test]
fn half_pie_splits_right_and_left_with_two_colors_per_cell() {
    // 0..0.5 starts at the top and goes clockwise → right half.
    let cells = pie_cells(16, 8, &[(0.5, Color::Red), (0.5, Color::Blue)], 0.0, 2.0);
    for row in &cells {
        for (col, cell) in row.iter().enumerate() {
            if let Some((_, pc)) = cell {
                if col >= 9 {
                    assert_eq!(pc.fg, Color::Red, "col {col}");
                }
                if col <= 6 {
                    assert_eq!(pc.fg, Color::Blue, "col {col}");
                }
            }
        }
    }
    // No interior cell is left with holes: where there are two colors, the second goes to the background.
    for row in &cells[2..6] {
        for cell in &row[6..10] {
            let (ch, pc) = cell.expect("interior ple");
            assert!(ch == '█' || pc.bg.is_some(), "{ch:?} {pc:?}");
        }
    }
}

#[test]
fn area_is_opaque_and_fills_from_bottom() {
    let full = area_cells(&[1.0], 1, 1, false);
    assert_eq!(full[0][0].map(|c| c.0), Some('█'));
    assert_eq!(area_cells(&[0.0], 1, 1, false)[0][0], None);
    assert_eq!(
        area_cells(&[0.5], 1, 1, false)[0][0].map(|c| c.0),
        Some('▄')
    );
    assert_eq!(
        area_cells(&[0.25], 1, 1, false)[0][0].map(|c| c.0),
        Some('▂')
    );
    // Two rows: half fills the bottom one entirely.
    let two = area_cells(&[0.5], 1, 2, false);
    assert_eq!(two[0][0], None);
    assert_eq!(two[1][0].map(|c| c.0), Some('█'));
    let cells = area_cells(&[0.3, 0.7, 1.0], 3, 4, false);
    assert!(
        cells
            .iter()
            .flatten()
            .flatten()
            .all(|(ch, _)| !is_braille(*ch))
    );
}

#[test]
fn inverted_area_fills_from_top_with_reversed_blocks() {
    // Half full from the top: inverted lower block → the top half is painted.
    let (ch, ac) = area_cells(&[0.5], 1, 1, true)[0][0].unwrap();
    assert_eq!(ch, '▄');
    assert!(ac.reversed);
    let (ch, ac) = area_cells(&[1.0], 1, 1, true)[0][0].unwrap();
    assert_eq!(ch, '█');
    assert!(!ac.reversed);
    let two = area_cells(&[0.5], 1, 2, true);
    assert_eq!(two[0][0].map(|c| c.0), Some('█'));
    assert_eq!(two[1][0], None);
}

#[test]
fn area_aligns_right_and_keeps_last_values() {
    let right = area_cells(&[1.0], 2, 1, false);
    assert_eq!(right[0][0], None);
    assert_eq!(right[0][1].map(|c| c.0), Some('█'));
    let mut values = vec![1.0; 10];
    values.push(0.0);
    let cells = area_cells(&values, 1, 1, false);
    assert_eq!(cells[0][0], None);
}

#[test]
fn area_levels_go_from_bottom_to_top() {
    let cells = area_cells(&[1.0], 1, 2, false);
    let top = cells[0][0].unwrap().1.level;
    let bottom = cells[1][0].unwrap().1.level;
    assert!(top > bottom);
}

#[test]
fn vbar_symbols() {
    assert_eq!(vbar_symbol(0.5, 1, 0), '▄');
    assert_eq!(vbar_symbol(1.0, 2, 1), '█');
    assert_eq!(vbar_symbol(0.25, 2, 1), ' ');
    assert_eq!(vbar_symbol(0.25, 2, 0), '▄');
}

#[test]
fn hbar_line_widths() {
    let line = hbar_line(0.5, 10, Color::Red, Color::Gray);
    assert_eq!(line.width(), 10);
    assert_eq!(line.spans[0].content.chars().count(), 5);
}

#[test]
fn pie_widget_centers_label_and_is_round() {
    let theme = Theme::new(true, 60.0, 80.0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    Pie {
        segments: &[(0.34, theme.ok), (0.66, theme.empty)],
        hole: 0.6,
        label: Some(("34%", theme.fg)),
        aspect: 2.2,
    }
    .render(buf.area, &mut buf);
    let row: String = (0..40).map(|x| buf[(x, 5)].symbol().to_string()).collect();
    assert!(row.contains("34%"), "{row}");
    // Drawn width = round(10 × 2.2) = 22 columns, centered.
    let used: Vec<u16> = (0..40)
        .filter(|x| (0..10).any(|y| buf[(*x, y)].symbol() != " "))
        .collect();
    assert_eq!(used.first(), Some(&9));
    assert_eq!(used.last(), Some(&30));
}

#[test]
fn area_widget_paints_fixed_color_and_reverses_when_inverted() {
    let theme = Theme::new(true, 60.0, 80.0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
    Area {
        values: &[1.0, 1.0],
        invert: false,
        color: AreaColor::Fixed(Color::Cyan),
        theme: &theme,
    }
    .render(buf.area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "█");
    assert_eq!(buf[(1, 0)].fg, Color::Cyan);

    let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
    Area {
        values: &[0.5],
        invert: true,
        color: AreaColor::Fixed(Color::Cyan),
        theme: &theme,
    }
    .render(buf.area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "▄");
    assert!(buf[(0, 0)].modifier.contains(Modifier::REVERSED));
}
