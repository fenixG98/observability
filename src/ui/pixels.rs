//! Pixel graphics (Kitty protocol or Sixel) for the donuts: real antialiasing instead
//! of text blocks. Only enabled when the terminal advertises a graphics protocol.

use image::{DynamicImage, RgbaImage};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    style::{Color, Style},
    widgets::Widget,
};
use ratatui_image::{
    Image, Resize,
    picker::{Capability, Picker, ProtocolType},
    protocol::{Protocol, kitty::Kitty},
};

/// Access to the terminal's graphics protocol. It is `Copy` because it lives inside `Theme`.
#[derive(Clone, Copy)]
pub struct Graphics {
    picker: &'static Picker,
}

impl Graphics {
    /// Only makes sense if the protocol isn't text blocks; see [`Graphics::detect`].
    pub fn new(picker: Picker) -> Graphics {
        Graphics {
            picker: Box::leak(Box::new(picker)),
        }
    }

    /// Asks the terminal which graphics protocol it supports. Must be called with the terminal already in
    /// raw mode and before reading events. `None` if only text blocks are available.
    pub fn detect() -> Option<Graphics> {
        let picker = Picker::from_query_stdio().ok()?;
        (picker.protocol_type() != ProtocolType::Halfblocks).then(|| Graphics::new(picker))
    }

    /// Cell size in pixels according to the terminal (width, height).
    pub fn font_size(&self) -> (u16, u16) {
        let fs = self.picker.font_size();
        (fs.width, fs.height)
    }

    /// Draws `img` occupying exactly `area`. `id` identifies the image in the terminal: with the
    /// same id, the new image replaces the previous one instead of piling up in memory.
    pub fn render(&self, img: RgbaImage, id: u32, area: Rect, buf: &mut Buffer) {
        let size = Size::new(area.width, area.height);
        let image = DynamicImage::ImageRgba8(img);
        let protocol = match self.picker.protocol_type() {
            ProtocolType::Kitty => {
                let compress = self
                    .picker
                    .capabilities()
                    .contains(&Capability::KittyCompression);
                Kitty::new(image, size, id, self.picker.tmux_detected(), compress)
                    .map(Protocol::Kitty)
                    .ok()
            }
            _ => self
                .picker
                .new_protocol(image, size, Resize::Fit(None))
                .ok(),
        };
        if let Some(p) = protocol {
            Image::new(&p).render(area, buf);
        }
    }

    /// Like [`Graphics::render`], with centered text over the image (the donut's %).
    pub fn render_labeled(
        &self,
        img: RgbaImage,
        id: u32,
        area: Rect,
        buf: &mut Buffer,
        label: Option<(&str, Style)>,
    ) {
        self.render(img, id, area, buf);
        let Some((text, style)) = label else { return };
        let tw = text.chars().count() as u16;
        let (lx, ly) = (
            area.x + area.width.saturating_sub(tw) / 2,
            area.y + area.height / 2,
        );
        buf.set_string(lx, ly, text, style);
        if self.picker.protocol_type() == ProtocolType::Kitty {
            relocate_kitty_cell(buf, area, lx + tw, ly);
        }
    }
}

/// Kitty placeholders only carry row and column in the first cell of each row; the
/// rest are inferred from the cell to the left. Text in the middle breaks the chain, so the
/// cell that follows it must state row and column explicitly again.
fn relocate_kitty_cell(buf: &mut Buffer, area: Rect, x: u16, y: u16) {
    if x >= area.right() || y >= area.bottom() {
        return;
    }
    let first: Vec<char> = buf[(area.x, y)].symbol().chars().collect();
    let n = first.len();
    if n < 4 || first[n - 4] != KITTY_PLACEHOLDER {
        return;
    }
    let (row, id_extra, fg) = (first[n - 3], first[n - 1], buf[(area.x, y)].fg);
    let Some(col) = kitty_diacritic(x - area.x) else {
        return;
    };
    let symbol: String = [KITTY_PLACEHOLDER, row, col, id_extra].iter().collect();
    buf[(x, y)].set_symbol(&symbol).set_fg(fg);
}

const KITTY_PLACEHOLDER: char = '\u{10EEEE}';

/// Diacritic that encodes `n` in Kitty placeholders. The table is private to ratatui-image,
/// so it is obtained by drawing an empty image of one column and `n + 1` rows: the cell of
/// row `n` carries the diacritic of `n`.
fn kitty_diacritic(n: u16) -> Option<char> {
    let size = Size::new(1, n + 1);
    let kitty = Kitty::new(DynamicImage::new_rgba8(1, 1), size, 1, false, false).ok()?;
    let area = Rect::new(0, 0, 1, n + 1);
    let mut probe = Buffer::empty(area);
    Image::new(&Protocol::Kitty(kitty)).render(area, &mut probe);
    let chars: Vec<char> = probe[(0, n)].symbol().chars().collect();
    (chars.len() >= 4).then(|| chars[chars.len() - 3])
}

impl std::fmt::Debug for Graphics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Graphics({:?})", self.picker.protocol_type())
    }
}

impl PartialEq for Graphics {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.picker, other.picker)
    }
}

/// Stable image id for a donut based on its screen position (never 0).
pub fn pie_image_id(area: Rect) -> u32 {
    0x0100_0000 | (u32::from(area.x) << 12) | u32::from(area.y & 0x0fff)
}

fn rgb(c: Color) -> Option<[f64; 3]> {
    match c {
        Color::Rgb(r, g, b) => Some([r as f64, g as f64, b as f64]),
        _ => None,
    }
}

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Donut (or pie if `hole == 0`) with antialiasing, centered and with the diameter of the short
/// side of the image. Transparent background. Angle 0 is at the top and grows clockwise.
/// `None` if there is no data, the size is zero or some color is not RGB.
pub fn donut_rgba(
    width: u32,
    height: u32,
    segments: &[(f64, Color)],
    hole: f64,
) -> Option<RgbaImage> {
    let total: f64 = segments.iter().map(|s| s.0.max(0.0)).sum();
    if width == 0 || height == 0 || total <= 0.0 || !total.is_finite() {
        return None;
    }
    let mut bounds: Vec<(f64, f64, [f64; 3])> = Vec::new();
    let mut acc = 0.0;
    for (f, c) in segments {
        let f = f.max(0.0) / total;
        if f > 0.0 {
            bounds.push((acc, acc + f, rgb(*c)?));
        }
        acc += f;
    }
    let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
    let outer = cx.min(cy) - 0.5;
    let inner = outer * hole.clamp(0.0, 0.99);
    let n = bounds.len();
    let mut img = RgbaImage::new(width, height);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
        let d = (dx * dx + dy * dy).sqrt();
        // Radial coverage with a 1 px band.
        let mut alpha = (outer - d + 0.5).clamp(0.0, 1.0);
        if hole > 0.0 {
            alpha = alpha.min((d - inner + 0.5).clamp(0.0, 1.0));
        }
        if alpha <= 0.0 {
            continue;
        }
        let angle = (dx.atan2(-dy) / std::f64::consts::TAU).rem_euclid(1.0);
        let i = bounds
            .iter()
            .position(|(a, b, _)| angle >= *a && angle < *b)
            .unwrap_or(n - 1);
        let mut color = bounds[i].2;
        if n > 1 {
            // Antialiasing at the edges between segments: pixel distance to the arc.
            let circ = std::f64::consts::TAU * d.max(1.0);
            let to_start = (angle - bounds[i].0) * circ;
            let to_end = (bounds[i].1 - angle) * circ;
            if to_start < 0.5 {
                let prev = bounds[(i + n - 1) % n].2;
                color = mix(prev, color, 0.5 + to_start);
            } else if to_end < 0.5 {
                let next = bounds[(i + 1) % n].2;
                color = mix(next, color, 0.5 + to_end);
            }
        }
        *p = image::Rgba([
            color[0].round() as u8,
            color[1].round() as u8,
            color[2].round() as u8,
            (alpha * 255.0).round() as u8,
        ]);
    }
    Some(img)
}
