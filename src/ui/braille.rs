//! Opaque block rasterization: pies (2×2 quadrants per cell), areas and bars.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::ui::pixels::{Graphics, donut_rgba, pie_image_id};
use crate::ui::theme::Theme;

/// `[row][column]` grid of optional cells with associated data.
pub type CellGrid<T> = Vec<Vec<Option<(char, T)>>>;

/// Quadrant character for a 4-bit mask: UL=1, UR=2, LL=4, LR=8.
pub fn quadrant(bits: u8) -> char {
    const Q: [char; 16] = [
        ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
    ];
    Q[(bits & 0x0f) as usize]
}

/// Colors of a pie cell: the marked quadrants use `fg`; the rest use `bg`
/// (or the terminal background if `None`). This way a cell can hold two segments without gaps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PieCell {
    pub fg: Color,
    pub bg: Option<Color>,
}

/// Size in cells (width, height) of the largest pie that fits in the area and is an exact circle
/// for cells of aspect ratio `aspect` (height/width).
pub fn pie_size(area_w: u16, area_h: u16, aspect: f64) -> (u16, u16) {
    if area_w == 0 || area_h == 0 {
        return (0, 0);
    }
    let w = (area_h as f64 * aspect).round() as u16;
    if w <= area_w {
        (w.max(1), area_h)
    } else {
        let h = (area_w as f64 / aspect).round().max(1.0) as u16;
        (area_w, h.min(area_h))
    }
}

/// Pie (or donut if `hole > 0`, empty fraction of the radius) with opaque quadrant blocks.
/// `aspect` is the cell height/width ratio: the geometry is computed in physical units
/// so the result is an exact circle. Angle 0 is at the top and grows clockwise.
/// `[row][column]`.
pub fn pie_cells(
    width: u16,
    height: u16,
    segments: &[(f64, Color)],
    hole: f64,
    aspect: f64,
) -> CellGrid<PieCell> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![vec![None; w]; h];
    let total: f64 = segments.iter().map(|s| s.0.max(0.0)).sum();
    if total <= 0.0 || w == 0 || h == 0 || !aspect.is_finite() || aspect <= 0.0 {
        return out;
    }
    // Units: cell width = 1; cell height = aspect.
    let (cx, cy) = (w as f64 / 2.0, h as f64 * aspect / 2.0);
    let radius = cx.min(cy);
    let mut bounds = Vec::new();
    let mut acc = 0.0;
    for (f, c) in segments {
        let f = f.max(0.0) / total;
        bounds.push((acc, acc + f, *c));
        acc += f;
    }
    let color_at = |px: f64, py: f64| -> Option<Color> {
        let (dx, dy) = (px - cx, py - cy);
        let d = (dx * dx + dy * dy).sqrt();
        if d > radius || d < radius * hole {
            return None;
        }
        let angle = (dx.atan2(-dy) / std::f64::consts::TAU).rem_euclid(1.0);
        bounds
            .iter()
            .find(|(a, b, _)| angle >= *a && angle < *b)
            .or(bounds.last())
            .map(|(_, _, c)| *c)
    };
    for (row, out_row) in out.iter_mut().enumerate() {
        for (col, cell) in out_row.iter_mut().enumerate() {
            // Subpixels in order UL, UR, LL, LR.
            let subs: [Option<Color>; 4] = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(sx, sy)| {
                color_at(
                    col as f64 + (sx as f64 + 0.5) / 2.0,
                    (row as f64 + (sy as f64 + 0.5) / 2.0) * aspect,
                )
            });
            let mut votes: Vec<(Color, u32)> = Vec::new();
            for c in subs.iter().flatten() {
                match votes.iter_mut().find(|v| v.0 == *c) {
                    Some(v) => v.1 += 1,
                    None => votes.push((*c, 1)),
                }
            }
            let Some(&(fg, _)) = votes.iter().max_by_key(|v| v.1) else {
                continue;
            };
            let empty = subs.iter().filter(|c| c.is_none()).count() as u32;
            let other = votes
                .iter()
                .filter(|v| v.0 != fg)
                .max_by_key(|v| v.1)
                .copied();
            // The background is the second color if there are more of it than empty ones; otherwise the terminal's.
            let bg = other.filter(|o| o.1 > empty).map(|o| o.0);
            let bits = subs
                .iter()
                .enumerate()
                .filter(|(_, c)| **c == Some(fg))
                .fold(0u8, |b, (i, _)| b | (1 << i));
            *cell = Some((quadrant(bits), PieCell { fg, bg }));
        }
    }
    out
}

/// Data for an area cell: `level` is the row position (0 = base, 1 = tip) used for
/// coloring; `reversed` means the character must be drawn in reverse video (areas that
/// grow from the top).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaCell {
    pub level: f64,
    pub reversed: bool,
}

/// Filled area of a series (values 0..=1, most recent last, right-aligned) with
/// opaque vertical blocks of 8 levels per cell. One sample per column.
pub fn area_cells(values: &[f64], width: u16, height: u16, invert: bool) -> CellGrid<AreaCell> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![vec![None; w]; h];
    if w == 0 || h == 0 {
        return out;
    }
    let start = values.len().saturating_sub(w);
    let shown = &values[start..];
    let offset = w - shown.len();
    for (i, v) in shown.iter().enumerate() {
        let col = offset + i;
        let v = if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let eighths = (v * (h * 8) as f64).round() as usize;
        for k in 0..h {
            let fill = eighths.saturating_sub(k * 8).min(8);
            if fill == 0 {
                break;
            }
            let row = if invert { k } else { h - 1 - k };
            let level = (k as f64 + 0.5) / h as f64;
            let (ch, reversed) = if fill == 8 {
                ('█', false)
            } else if invert {
                // Inverted lower block: the top part ends up painted.
                (BLOCKS[8 - fill], true)
            } else {
                (BLOCKS[fill], false)
            };
            out[row][col] = Some((ch, AreaCell { level, reversed }));
        }
    }
    out
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

/// Pie/donut widget, centered and circular for cells of aspect ratio `aspect`
/// (height/width; see `Theme::cell_aspect`).
pub struct Pie<'a> {
    pub segments: &'a [(f64, Color)],
    pub hole: f64,
    pub label: Option<(&'a str, Color)>,
    pub aspect: f64,
    /// If present, the donut is drawn with pixels (antialiasing); otherwise with quadrants.
    pub graphics: Option<Graphics>,
    /// Cell size in pixels, needed for pixel drawing.
    pub cell_px: Option<(u16, u16)>,
}

/// Summary of everything that determines a donut's image. Fractions are rounded to the
/// thousandth: a smaller change would not be visible and isn't worth retransmitting the image.
fn pie_key(area: Rect, cell_px: (u16, u16), segments: &[(f64, Color)], hole: f64) -> Vec<u64> {
    let mut key = vec![
        u64::from(area.x),
        u64::from(area.y),
        u64::from(area.width),
        u64::from(area.height),
        u64::from(cell_px.0),
        u64::from(cell_px.1),
        hole.to_bits(),
    ];
    for (f, c) in segments {
        key.push((f.max(0.0) * 1000.0).round() as u64);
        key.push(match c {
            Color::Rgb(r, g, b) => u64::from_be_bytes([0, 0, 0, 0, 0, *r, *g, *b]),
            // Without RGB no image is generated (drawn with text) and nothing is stored.
            _ => 0,
        });
    }
    key
}

impl Widget for Pie<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let (Some(g), Some((cw, ch))) = (self.graphics, self.cell_px)
            && area.width > 0
            && area.height > 0
            && g.render_cached(
                pie_image_id(area),
                pie_key(area, (cw, ch), self.segments, self.hole),
                area,
                buf,
                || {
                    donut_rgba(
                        u32::from(area.width) * u32::from(cw),
                        u32::from(area.height) * u32::from(ch),
                        self.segments,
                        self.hole,
                    )
                },
            )
        {
            let label = self
                .label
                .map(|(t, c)| (t, Style::new().fg(c).add_modifier(Modifier::BOLD)));
            g.draw_label(area, buf, label);
            return;
        }
        let (w, h) = pie_size(area.width, area.height, self.aspect);
        if w == 0 || h == 0 {
            return;
        }
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - h) / 2;
        for (r, row) in pie_cells(w, h, self.segments, self.hole, self.aspect)
            .iter()
            .enumerate()
        {
            for (c, cell) in row.iter().enumerate() {
                if let Some((ch, pc)) = cell {
                    let cell = &mut buf[(x0 + c as u16, y0 + r as u16)];
                    cell.set_char(*ch).set_fg(pc.fg);
                    if let Some(bg) = pc.bg {
                        cell.set_bg(bg);
                    }
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
                if let Some((ch, ac)) = cell {
                    let color = match self.color {
                        AreaColor::Fixed(c) => c,
                        AreaColor::Level => self.theme.level(ac.level),
                    };
                    let cell = &mut buf[(area.x + c as u16, area.y + r as u16)];
                    cell.set_char(*ch).set_fg(color);
                    if ac.reversed {
                        cell.modifier.insert(Modifier::REVERSED);
                    }
                }
            }
        }
    }
}
