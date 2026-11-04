//! Compact view for small terminals: a single panel with bars and sparklines.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::model::{Avail, human_bytes, human_rate};
use crate::ui::braille::{Area, AreaColor, hbar_line};
use crate::ui::theme::{Theme, truncate};

const LABEL_W: u16 = 9;
const VALUE_W: u16 = 15;

fn row(
    label: &str,
    fraction: f64,
    fill: Color,
    value: &str,
    bar_w: u16,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{:<9}", truncate(label, 8)),
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
    )];
    spans.extend(hbar_line(fraction, bar_w, fill, theme.empty).spans);
    spans.push(Span::styled(
        format!(" {:<14}", truncate(value, 14)),
        Style::new().fg(theme.dim),
    ));
    Line::from(spans)
}

pub fn render(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = theme.panel("RESUM", "vista compacta", false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < LABEL_W + 6 || inner.height == 0 {
        return;
    }
    let s = &app.snapshot;
    let spark_w = inner
        .width
        .saturating_sub(LABEL_W + VALUE_W)
        .saturating_sub(inner.width / 3);
    let bar_w = inner
        .width
        .saturating_sub(LABEL_W + VALUE_W + spark_w)
        .max(4);
    let mut lines: Vec<Line> = Vec::new();
    let mut sparks: Vec<(usize, Vec<f64>, AreaColor)> = Vec::new();

    let cpu = (s.cpu.total as f64 / 100.0).clamp(0.0, 1.0);
    sparks.push((lines.len(), app.hist.cpu.values(), AreaColor::Level));
    lines.push(row(
        "CPU",
        cpu,
        theme.level(cpu),
        &format!("{:.0}%", s.cpu.total),
        bar_w,
        theme,
    ));

    let label_style = Style::new().fg(theme.fg).add_modifier(Modifier::BOLD);
    match app.current_gpu() {
        Some(g) => match &g.util {
            Avail::Ok(u) => {
                let t = (*u as f64 / 100.0).clamp(0.0, 1.0);
                if let Some(h) = app.hist.gpu.get(app.gpu_index) {
                    sparks.push((lines.len(), h.values(), AreaColor::Fixed(theme.palette[0])));
                }
                lines.push(row(
                    "GPU",
                    t,
                    theme.level(t),
                    &format!("{u:.0}%"),
                    bar_w,
                    theme,
                ));
            }
            Avail::Unavailable(r) => lines.push(Line::from(vec![
                Span::styled(format!("{:<9}", "GPU"), label_style),
                Span::styled(
                    truncate(r, inner.width.saturating_sub(LABEL_W) as usize),
                    Style::new().fg(theme.warm),
                ),
            ])),
        },
        None => lines.push(Line::from(vec![
            Span::styled(format!("{:<9}", "GPU"), label_style),
            Span::styled(
                s.gpus.reason().unwrap_or("cap GPU").to_string(),
                Style::new().fg(theme.warm),
            ),
        ])),
    }

    let m = &s.mem;
    let used = m.used_fraction();
    lines.push(row(
        "MEM",
        used,
        theme.level(used),
        &format!("{:.0}% de {}", used * 100.0, human_bytes(m.total)),
        bar_w,
        theme,
    ));
    let swap = if m.swap_total == 0 {
        0.0
    } else {
        m.swap_used as f64 / m.swap_total as f64
    };
    lines.push(row(
        "SWAP",
        swap,
        theme.palette[3],
        &format!("{}/{}", human_bytes(m.swap_used), human_bytes(m.swap_total)),
        bar_w,
        theme,
    ));

    if let Some(d) = s.disk.mounts.first() {
        let t = if d.total == 0 {
            0.0
        } else {
            d.used as f64 / d.total as f64
        };
        lines.push(row(
            &format!("DISC {}", d.mount),
            t,
            theme.palette[3],
            &format!("{}/{}", human_bytes(d.used), human_bytes(d.total)),
            bar_w,
            theme,
        ));
    }

    let rx_scale = app.hist.rx.max().max(1.0);
    sparks.push((
        lines.len(),
        app.hist.rx.values().iter().map(|v| v / rx_scale).collect(),
        AreaColor::Fixed(theme.rx),
    ));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<9}", "↓ xarxa"),
            Style::new().fg(theme.rx).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ({})", human_rate(s.net.rx_bps), s.net.iface_label),
            Style::new().fg(theme.dim),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<9}", "↑ xarxa"),
            Style::new().fg(theme.tx).add_modifier(Modifier::BOLD),
        ),
        Span::styled(human_rate(s.net.tx_bps), Style::new().fg(theme.dim)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<9}", "disc I/O"), label_style),
        Span::styled(
            format!(
                "r {}  w {}",
                human_rate(s.disk.read_bps),
                human_rate(s.disk.write_bps)
            ),
            Style::new().fg(theme.dim),
        ),
    ]));

    match &s.temps {
        Avail::Ok(list) => {
            for t in list.iter().take(4) {
                lines.push(row(
                    &t.label,
                    t.celsius as f64 / 100.0,
                    theme.temp(t.celsius),
                    &format!("{:.0}°C", t.celsius),
                    bar_w,
                    theme,
                ));
            }
        }
        Avail::Unavailable(r) => lines.push(Line::from(Span::styled(
            format!("temperatures: {r}"),
            Style::new().fg(theme.warm),
        ))),
    }

    f.render_widget(Paragraph::new(lines), inner);

    if spark_w >= 4 {
        let x = inner.x + inner.width - spark_w;
        for (line, values, color) in sparks {
            if line as u16 >= inner.height {
                continue;
            }
            f.render_widget(
                Area {
                    values: &values,
                    invert: false,
                    color,
                    theme,
                },
                Rect::new(x, inner.y + line as u16, spark_w, 1),
            );
        }
    }
}
