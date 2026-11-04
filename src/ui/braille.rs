//! Rasterization with braille characters (2×4 dots per cell): pies, areas and bars.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::ui::theme::Theme;

/// Bit of each dot: `BITS[column][row]`.
const BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// `[row][column]` grid of optional braille cells with associated data.
pub type CellGrid<T> = Vec<Vec<Option<(char, T)>>>;

fn braille(bits: u8) -> char {
    char::from_u32(0x2800 + bits as u32).unwrap_or(' ')
}

/// Pie (or donut if `hole > 0`, empty fraction of the radius). `[row][column]`.
/// Angle 0 is at the top and grows clockwise. Each cell takes the majority color.
pub fn pie_cells(width: u16, height: u16, segments: &[(f64, Color)], hole: f64) -> CellGrid<Color> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![vec![None; w]; h];
    let total: f64 = segments.iter().map(|s| s.0.max(0.0)).sum();
    if total <= 0.0 || w == 0 || h == 0 {
        return out;
    }
    let radius = (w * 2).min(h * 4) as f64 / 2.0;
    let (cx, cy) = (w as f64, h as f64 * 2.0);
    let mut bounds = Vec::new();
    let mut acc = 0.0;
    for (f, c) in segments {
        let f = f.max(0.0) / total;
        bounds.push((acc, acc + f, *c));
        acc += f;
    }
    for (row, out_row) in out.iter_mut().enumerate() {
        for (col, cell) in out_row.iter_mut().enumerate() {
            let mut bits = 0u8;
            let mut votes: Vec<(Color, u32)> = Vec::new();
            for (dx, dot_col) in BITS.iter().enumerate() {
                for (dy, bit) in dot_col.iter().enumerate() {
                    let px = (col * 2 + dx) as f64 + 0.5 - cx;
                    let py = (row * 4 + dy) as f64 + 0.5 - cy;
                    let d = (px * px + py * py).sqrt();
                    if d > radius || d < radius * hole {
                        continue;
                    }
                    let angle = (px.atan2(-py) / std::f64::consts::TAU).rem_euclid(1.0);
                    let color = bounds
                        .iter()
                        .find(|(a, b, _)| angle >= *a && angle < *b)
                        .or(bounds.last())
                        .map(|(_, _, c)| *c);
                    if let Some(c) = color {
                        bits |= bit;
                        match votes.iter_mut().find(|v| v.0 == c) {
                            Some(v) => v.1 += 1,
                            None => votes.push((c, 1)),
                        }
                    }
                }
            }
            if let Some((color, _)) = votes.iter().max_by_key(|v| v.1) {
                *cell = Some((braille(bits), *color));
            }
        }
    }
    out
}

/// Filled area of a series (values 0..=1, most recent last, right-aligned).
/// Returns the character and the row level (0 = base, 1 = tip) for coloring.
pub fn area_cells(values: &[f64], width: u16, height: u16, invert: bool) -> CellGrid<f64> {
    let (w, h) = (width as usize, height as usize);
    let mut bits = vec![vec![0u8; w]; h];
    let dots_h = h * 4;
    let n = w * 2;
    let start = values.len().saturating_sub(n);
    let shown = &values[start..];
    let offset = n - shown.len();
    for (i, v) in shown.iter().enumerate() {
        let x = offset + i;
        let (col, dx) = (x / 2, x % 2);
        let level = (v.clamp(0.0, 1.0) * dots_h as f64).round() as usize;
        for k in 0..level {
            let y = if invert { k } else { dots_h - 1 - k };
            bits[y / 4][col] |= BITS[dx][y % 4];
        }
    }
    bits.iter()
        .enumerate()
        .map(|(row, r)| {
            let lvl = if invert {
                (row as f64 + 0.5) / h as f64
            } else {
                1.0 - (row as f64 + 0.5) / h as f64
            };
            r.iter()
                .map(|b| (*b != 0).then(|| (braille(*b), lvl)))
                .collect()
        })
        .collect()
}

pub const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Character of a vertical bar of `rows` rows for row `row_from_bottom`.
pub fn vbar_symbol(fraction: f64, rows: u16, row_from_bottom: u16) -> char {
    let eighths = (fraction.clamp(0.0, 1.0) * rows as f64 * 8.0).round() as i64;
    let in_cell = (eighths - row_from_bottom as i64 * 8).clamp(0, 8) as usize;
    BLOCKS[in_cell]
}

/// Horizontal bar of squares: filled part with `fill`, rest with `empty`.
pub fn hbar_line(fraction: f64, width: u16, fill: Color, empty: Color) -> Line<'static> {
    let w = width as usize;
    let n = ((fraction.clamp(0.0, 1.0) * w as f64).round() as usize).min(w);
    Line::from(vec![
        Span::styled("■".repeat(n), Style::new().fg(fill)),
        Span::styled("■".repeat(w - n), Style::new().fg(empty)),
    ])
}

/// Pie/donut widget, centered and with circle proportion (width = 2 × height).
pub struct Pie<'a> {
    pub segments: &'a [(f64, Color)],
    pub hole: f64,
    pub label: Option<(&'a str, Color)>,
}

impl Widget for Pie<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height == 0 {
            return;
        }
        let h = area.height.min(area.width / 2).max(1);
        let w = (h * 2).min(area.width);
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - h) / 2;
        for (r, row) in pie_cells(w, h, self.segments, self.hole).iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if let Some((ch, color)) = cell {
                    buf[(x0 + c as u16, y0 + r as u16)]
                        .set_char(*ch)
                        .set_fg(*color);
                }
            }
        }
        if let Some((text, color)) = self.label {
            let tw = text.chars().count() as u16;
            let tx = x0 + w.saturating_sub(tw) / 2;
            let ty = y0 + h / 2;
            buf.set_string(
                tx,
                ty,
                text,
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AreaColor {
    Fixed(Color),
    /// Color by height (theme green/yellow/red).
    Level,
}

/// Braille area widget. `invert` draws from the top downward.
pub struct Area<'a> {
    pub values: &'a [f64],
    pub invert: bool,
    pub color: AreaColor,
    pub theme: &'a Theme,
}

impl Widget for Area<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (r, row) in area_cells(self.values, area.width, area.height, self.invert)
            .iter()
            .enumerate()
        {
            for (c, cell) in row.iter().enumerate() {
                if let Some((ch, level)) = cell {
                    let color = match self.color {
                        AreaColor::Fixed(c) => c,
                        AreaColor::Level => self.theme.level(*level),
                    };
                    buf[(area.x + c as u16, area.y + r as u16)]
                        .set_char(*ch)
                        .set_fg(color);
                }
            }
        }
    }
}
