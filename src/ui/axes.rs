//! Scaled axes for the history charts: values on the left and time at the bottom.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::ui::theme::Theme;

/// Draws the Y axis on the left of `area` and returns the space left for the chart.
/// `labels` are pairs (row from the top within `area`, text); a `┤`
/// mark goes on those rows. If it doesn't fit, draws nothing and returns the whole `area`.
pub fn y_axis(buf: &mut Buffer, area: Rect, labels: &[(u16, String)], theme: &Theme) -> Rect {
    let label_w = labels
        .iter()
        .map(|(_, l)| l.chars().count() as u16)
        .max()
        .unwrap_or(0);
    if area.height == 0 || area.width < label_w + 2 {
        return area;
    }
    let axis_x = area.x + label_w;
    let line = Style::new().fg(theme.border);
    let text = Style::new().fg(theme.dim);
    for dy in 0..area.height {
        let y = area.y + dy;
        let label = labels.iter().find(|(r, _)| *r == dy);
        buf[(axis_x, y)]
            .set_char(if label.is_some() { '┤' } else { '│' })
            .set_style(line);
        if let Some((_, l)) = label {
            let w = l.chars().count() as u16;
            buf.set_string(area.x + label_w - w, y, l, text);
        }
    }
    Rect::new(axis_x + 1, area.y, area.width - label_w - 1, area.height)
}

/// Draws the time axis in the first two rows of `area`: the line with ticks under the
/// columns of `plot` and, below, the relative time of each tick. Each chart column is
/// a sample of `step_secs` seconds; the rightmost one is «now».
pub fn x_axis(buf: &mut Buffer, area: Rect, plot: Rect, step_secs: f64, theme: &Theme) {
    if area.height == 0 || plot.width == 0 || plot.x < area.x || plot.right() > area.right() {
        return;
    }
    let line = Style::new().fg(theme.border);
    let text = Style::new().fg(theme.dim);
    let w = plot.width;
    let n = if w >= 40 {
        5
    } else if w >= 16 {
        3
    } else {
        2
    };
    // Ticks are spread out in time (round numbers) and then placed at the nearest column;
    // this way all charts of the same window show the same thing.
    let window = f64::from(w - 1) * step_secs;
    let ticks: Vec<(u16, f64)> = (0..n)
        .map(|i| {
            let secs = (window * f64::from(n - 1 - i) / f64::from(n - 1)).round();
            let back = if step_secs > 0.0 {
                (secs / step_secs).round() as u16
            } else {
                0
            };
            ((w - 1).saturating_sub(back), secs)
        })
        .collect();
    if plot.x > area.x {
        buf[(plot.x - 1, area.y)].set_char('└').set_style(line);
    }
    for c in 0..w {
        let ch = if ticks.iter().any(|(t, _)| *t == c) {
            '┴'
        } else {
            '─'
        };
        buf[(plot.x + c, area.y)].set_char(ch).set_style(line);
    }
    if area.height < 2 {
        return;
    }
    let y = area.y + 1;
    let mut free_from = area.x;
    for (c, secs) in ticks {
        let label = time_label(secs);
        let lw = label.chars().count() as u16;
        let center = plot.x + c;
        let start = center
            .saturating_sub(lw / 2)
            .max(area.x)
            .min(area.right().saturating_sub(lw));
        if start < free_from || start + lw > area.right() {
            continue;
        }
        buf.set_string(start, y, &label, text);
        free_from = start + lw + 1;
    }
}

/// Compact relative time: «now», «-45s», «-5m», «-2h».
pub fn time_label(secs_ago: f64) -> String {
    let s = secs_ago.round().max(0.0) as u64;
    if s == 0 {
        "now".into()
    } else if s < 120 {
        format!("-{s}s")
    } else if s < 7200 {
        format!("-{}m", s / 60)
    } else {
        format!("-{}h", s / 3600)
    }
}

/// Compact rate for axis labels (decimal units, without «/s»): «13M», «2.5K».
pub fn short_rate(bps: f64) -> String {
    let bps = if bps.is_finite() { bps.max(0.0) } else { 0.0 };
    let (v, unit) = if bps >= 1e9 {
        (bps / 1e9, "G")
    } else if bps >= 1e6 {
        (bps / 1e6, "M")
    } else if bps >= 1e3 {
        (bps / 1e3, "K")
    } else if bps >= 1.0 {
        return format!("{bps:.0}B");
    } else {
        return "0".into();
    };
    if v < 10.0 {
        format!("{v:.1}{unit}")
    } else {
        format!("{v:.0}{unit}")
    }
}

/// Resamples a series so it fills `width` columns that represent the whole window of
/// the history (`capacity` samples). Samples that don't exist yet are left as NaN
/// (not drawn) on the left. If there are more samples than columns, each column takes the maximum
/// of its span so peaks are not lost.
pub fn fit_series(values: &[f64], capacity: usize, width: u16) -> Vec<f64> {
    let w = width as usize;
    if w == 0 {
        return Vec::new();
    }
    if capacity == 0 {
        return vec![f64::NAN; w];
    }
    let shown = &values[values.len().saturating_sub(capacity)..];
    let missing = capacity - shown.len();
    let at = |i: usize| {
        if i < missing {
            f64::NAN
        } else {
            shown[i - missing]
        }
    };
    (0..w)
        .map(|c| {
            let (a, b) = (c * capacity / w, (c + 1) * capacity / w);
            if b <= a {
                return at(a.min(capacity - 1));
            }
            (a..b)
                .map(at)
                .filter(|v| v.is_finite())
                .fold(f64::NAN, f64::max)
        })
        .collect()
}
