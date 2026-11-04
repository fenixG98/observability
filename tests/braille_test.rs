use observability::ui::braille::*;
use observability::ui::theme::Theme;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

#[test]
fn pie_without_data_is_empty() {
    let cells = pie_cells(8, 4, &[], 0.0);
    assert!(cells.iter().flatten().all(|c| c.is_none()));
    let cells = pie_cells(8, 4, &[(0.0, Color::Red)], 0.0);
    assert!(cells.iter().flatten().all(|c| c.is_none()));
}

#[test]
fn full_pie_uses_one_color_and_hole_empties_center() {
    let cells = pie_cells(8, 4, &[(1.0, Color::Red)], 0.0);
    let filled: Vec<_> = cells.iter().flatten().flatten().collect();
    assert!(filled.len() > 20);
    assert!(filled.iter().all(|(_, c)| *c == Color::Red));
    assert!(cells[1][3].is_some() && cells[2][4].is_some());
    let donut = pie_cells(8, 4, &[(1.0, Color::Red)], 0.6);
    assert!(donut[1][3].is_none() && donut[2][4].is_none());
}

#[test]
fn half_pie_splits_right_and_left() {
    // 0..0.5 starts at the top and goes clockwise → right half.
    let cells = pie_cells(8, 4, &[(0.5, Color::Red), (0.5, Color::Blue)], 0.0);
    for row in &cells {
        for (col, cell) in row.iter().enumerate() {
            if let Some((_, c)) = cell {
                if col >= 5 {
                    assert_eq!(*c, Color::Red, "col {col}");
                }
                if col <= 2 {
                    assert_eq!(*c, Color::Blue, "col {col}");
                }
            }
        }
    }
}

#[test]
fn area_fills_from_bottom_and_aligns_right() {
    assert_eq!(
        area_cells(&[1.0, 1.0], 1, 1, false)[0][0].map(|c| c.0),
        Some('⣿')
    );
    assert_eq!(area_cells(&[0.0, 0.0], 1, 1, false)[0][0], None);
    assert_eq!(
        area_cells(&[0.5, 0.5], 1, 1, false)[0][0].map(|c| c.0),
        Some('⣤')
    );
    assert_eq!(
        area_cells(&[0.5, 0.5], 1, 1, true)[0][0].map(|c| c.0),
        Some('⠛')
    );
    let right = area_cells(&[1.0], 2, 1, false);
    assert_eq!(right[0][0], None);
    assert_eq!(right[0][1].map(|c| c.0), Some('⢸'));
}

#[test]
fn area_levels_go_from_bottom_to_top() {
    let cells = area_cells(&[1.0, 1.0], 1, 2, false);
    let top = cells[0][0].unwrap().1;
    let bottom = cells[1][0].unwrap().1;
    assert!(top > bottom);
}

#[test]
fn area_keeps_only_last_values_that_fit() {
    let mut values = vec![1.0; 10];
    values.extend([0.0, 0.0]);
    let cells = area_cells(&values, 1, 1, false);
    assert_eq!(cells[0][0], None);
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
fn pie_widget_centers_label() {
    let theme = Theme::new(true, 60.0, 80.0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
    Pie {
        segments: &[(0.34, theme.ok), (0.66, theme.empty)],
        hole: 0.6,
        label: Some(("34%", theme.fg)),
    }
    .render(buf.area, &mut buf);
    let row: String = (0..20).map(|x| buf[(x, 5)].symbol().to_string()).collect();
    assert!(row.contains("34%"), "{row}");
}

#[test]
fn area_widget_paints_fixed_color() {
    let theme = Theme::new(true, 60.0, 80.0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
    Area {
        values: &[1.0, 1.0, 1.0, 1.0],
        invert: false,
        color: AreaColor::Fixed(Color::Cyan),
        theme: &theme,
    }
    .render(buf.area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "⣿");
    assert_eq!(buf[(1, 0)].fg, Color::Cyan);
}
