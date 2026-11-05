//! Tab 1: layout "B · Pies on top".

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::app::{App, Panel};
use crate::model::{Avail, GpuReading, MemKind, human_bytes, human_rate};
use crate::ui::axes::{fit_series, short_rate, x_axis, y_axis};
use crate::ui::braille::{Area, AreaColor, Pie, hbar_line, pie_size, vbar_symbol};
use crate::ui::theme::{Theme, truncate};

/// "Intel(R) Core(TM) Ultra 5 125H" → "Core Ultra 5 125H".
pub fn short_model(model: &str) -> String {
    let cleaned = model
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace(" Processor", "")
        .replace(" CPU", "");
    let cleaned = cleaned
        .strip_prefix("Intel ")
        .unwrap_or(&cleaned)
        .to_string();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn render(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    if app.zoomed {
        render_panel(f, area, app.focus, app, theme);
        return;
    }
    let [top, mid, bottom] = Layout::vertical([
        Constraint::Percentage(38),
        Constraint::Percentage(34),
        Constraint::Fill(1),
    ])
    .areas(area);
    let cols = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(top);
    for (rect, panel) in cols.iter().zip([
        Panel::CpuDonut,
        Panel::MemDonut,
        Panel::GpuDonut,
        Panel::DiskDonut,
    ]) {
        render_panel(f, *rect, panel, app, theme);
    }
    let [hist, temps] =
        Layout::horizontal([Constraint::Percentage(66), Constraint::Fill(1)]).areas(mid);
    render_panel(f, hist, Panel::History, app, theme);
    render_panel(f, temps, Panel::Temps, app, theme);
    let [net, io] =
        Layout::horizontal([Constraint::Percentage(66), Constraint::Fill(1)]).areas(bottom);
    render_panel(f, net, Panel::Net, app, theme);
    render_panel(f, io, Panel::DiskIo, app, theme);
}

pub fn render_panel(f: &mut Frame, area: Rect, panel: Panel, app: &App, theme: &Theme) {
    let focused = app.focus == panel;
    match panel {
        Panel::CpuDonut => cpu_donut(f, area, app, theme, focused),
        Panel::MemDonut => mem_donut(f, area, app, theme, focused),
        Panel::GpuDonut => gpu_donut(f, area, app, theme, focused),
        Panel::DiskDonut => disk_donut(f, area, app, theme, focused),
        Panel::History => history(f, area, app, theme, focused),
        Panel::Temps => temps(f, area, app, theme, focused),
        Panel::Net => net(f, area, app, theme, focused),
        Panel::DiskIo => disk_io(f, area, app, theme, focused),
        Panel::ProcTable | Panel::ProcPie | Panel::ProcDetail => {}
    }
}

/// Explanatory text inside a panel (data not available).
fn message(f: &mut Frame, area: Rect, block: Block<'static>, text: &str, theme: &Theme) {
    f.render_widget(
        Paragraph::new(text.to_string())
            .style(Style::new().fg(theme.warm))
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

/// Writes dimmed `text` at the top-left corner of `area` (over the chart).
/// Seconds per column so the time axis covers the whole history window: the
/// left column is the oldest sample and the right one is «now».
fn window_step(app: &App, capacity: usize, width: u16) -> f64 {
    capacity as f64 * app.refresh.as_secs_f64() / f64::from(width.saturating_sub(1).max(1))
}

/// Labels 100 % / 50 % / 0 % for an axis of `h` rows (50 % only if it fits).
fn percent_labels(h: u16) -> Vec<(u16, String)> {
    let mut v = vec![(0, "100%".to_string())];
    if h >= 5 {
        v.push(((h - 1) / 2, "50%".into()));
    }
    if h >= 2 {
        v.push((h - 1, "0%".into()));
    }
    v
}

fn corner_label(f: &mut Frame, area: Rect, text: &str, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // One column of separation from the Y axis.
    let pad = u16::from(area.width > 2);
    let text = truncate(text, (area.width - pad) as usize);
    f.buffer_mut().set_string(
        area.x + pad,
        area.y,
        text,
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    );
}

struct Donut<'a> {
    segments: &'a [(f64, Color)],
    center: String,
    sub: String,
    legend: Vec<(Color, String)>,
}

fn donut(f: &mut Frame, area: Rect, block: Block<'static>, d: Donut, theme: &Theme) {
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width < 4 {
        return;
    }
    // The circle, the legend (if it fits) and the subtitle form a group centered in the panel:
    // one row of margin at the top and between the circle and the subtitle, and a fixed margin up to the legend.
    const LEGEND_GAP: u16 = 4;
    let margin = u16::from(inner.height >= 8);
    let pie_h = inner.height - 1 - 2 * margin;
    let legend_w = d
        .legend
        .iter()
        .map(|(_, l)| l.chars().count() as u16 + 2)
        .max()
        .unwrap_or(0);
    let full_w = (f64::from(pie_h) * theme.cell_aspect).round() as u16;
    let show_legend = legend_w > 0 && inner.width >= full_w + LEGEND_GAP + legend_w;
    let room = if show_legend {
        inner.width - LEGEND_GAP - legend_w
    } else {
        inner.width
    };
    let (pw, ph) = pie_size(room, pie_h, theme.cell_aspect);
    let group_w = if show_legend {
        pw + LEGEND_GAP + legend_w
    } else {
        pw
    };
    let x0 = inner.x + (inner.width - group_w) / 2;
    let y0 = inner.y + margin + (pie_h - ph) / 2;
    let pie_area = Rect::new(x0, y0, pw, ph);
    let text = Rect::new(
        inner.x,
        (y0 + ph + margin).min(inner.bottom() - 1),
        inner.width,
        1,
    );
    if show_legend {
        let legend = Rect::new(x0 + pw + LEGEND_GAP, y0, legend_w, ph);
        let lines: Vec<Line> = d
            .legend
            .iter()
            .map(|(c, label)| {
                Line::from(vec![
                    Span::styled("■ ", Style::new().fg(*c)),
                    Span::styled(label.clone(), Style::new().fg(theme.fg)),
                ])
            })
            .collect();
        let top = legend.y + legend.height.saturating_sub(lines.len() as u16) / 2;
        f.render_widget(
            Paragraph::new(lines),
            Rect::new(
                legend.x,
                top,
                legend.width,
                legend.height.min(d.legend.len() as u16),
            ),
        );
    }
    f.render_widget(
        Pie {
            segments: d.segments,
            hole: 0.6,
            label: Some((&d.center, theme.fg)),
            aspect: theme.cell_aspect,
            graphics: theme.graphics,
            cell_px: theme.cell_px,
        },
        pie_area,
    );
    f.render_widget(
        Paragraph::new(
            Line::from(Span::styled(
                truncate(&d.sub, text.width as usize),
                Style::new().fg(theme.dim),
            ))
            .centered(),
        ),
        text,
    );
}

fn cpu_donut(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let cpu = &app.snapshot.cpu;
    let t = (cpu.total as f64 / 100.0).clamp(0.0, 1.0);
    let sub = if cpu.freq_mhz > 0 {
        format!(
            "{} threads · {:.1} GHz",
            cpu.per_core.len(),
            cpu.freq_mhz as f64 / 1000.0
        )
    } else {
        format!("{} threads", cpu.per_core.len())
    };
    let segments = [(t, theme.level(t)), (1.0 - t, theme.empty)];
    donut(
        f,
        area,
        theme.panel("CPU", short_model(&cpu.model), focused),
        Donut {
            segments: &segments,
            center: format!("{:.0}%", t * 100.0),
            sub,
            legend: Vec::new(),
        },
        theme,
    );
}

fn mem_donut(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let mem = &app.snapshot.mem;
    let total = mem.total.max(1) as f64;
    let colors: Vec<Color> = mem
        .segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if s.kind == MemKind::Free {
                theme.empty
            } else {
                theme.palette[i % theme.palette.len()]
            }
        })
        .collect();
    let segments: Vec<(f64, Color)> = mem
        .segments
        .iter()
        .zip(&colors)
        .map(|(s, c)| (s.bytes as f64 / total, *c))
        .collect();
    let legend = mem
        .segments
        .iter()
        .zip(&colors)
        .map(|(s, c)| (*c, format!("{} {}", s.label, human_bytes(s.bytes))))
        .collect();
    let used: u64 = mem
        .segments
        .iter()
        .filter(|s| s.kind == MemKind::Used)
        .map(|s| s.bytes)
        .sum();
    let right = if mem.swap_total > 0 {
        format!(
            "swap {}/{}",
            human_bytes(mem.swap_used),
            human_bytes(mem.swap_total)
        )
    } else {
        String::new()
    };
    donut(
        f,
        area,
        theme.panel("MEMORY", right, focused),
        Donut {
            segments: &segments,
            center: format!("{:.0}%", mem.used_fraction() * 100.0),
            sub: format!("{} / {}", human_bytes(used), human_bytes(mem.total)),
            legend,
        },
        theme,
    );
}

fn gpu_sub(g: &GpuReading) -> String {
    let mut parts = Vec::new();
    if let (Some(u), Some(t)) = (g.vram_used, g.vram_total)
        && t > 0
    {
        parts.push(format!("VRAM {}/{}", human_bytes(u), human_bytes(t)));
    }
    if let Some(t) = g.temp_c {
        parts.push(format!("{t:.0}°C"));
    }
    if let Some(p) = g.power_w {
        parts.push(format!("{p:.0} W"));
    }
    if parts.is_empty() {
        g.name.clone()
    } else {
        parts.join(" · ")
    }
}

fn gpu_donut(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let n = app.gpu_count();
    let right = if n > 1 {
        format!("{}/{}", app.gpu_index + 1, n)
    } else {
        String::new()
    };
    match (&app.snapshot.gpus, app.current_gpu()) {
        (Avail::Unavailable(reason), _) => {
            message(f, area, theme.panel("GPU", right, focused), reason, theme)
        }
        (Avail::Ok(_), None) => message(
            f,
            area,
            theme.panel("GPU", right, focused),
            "no GPU detected",
            theme,
        ),
        (Avail::Ok(_), Some(g)) => match &g.util {
            Avail::Unavailable(reason) => message(
                f,
                area,
                theme.panel(format!("GPU · {}", truncate(&g.name, 18)), right, focused),
                reason,
                theme,
            ),
            Avail::Ok(u) => {
                let t = (*u as f64 / 100.0).clamp(0.0, 1.0);
                let segments = [(t, theme.level(t)), (1.0 - t, theme.empty)];
                let legend = g
                    .engines
                    .iter()
                    .map(|(e, v)| (theme.palette[0], format!("{e} {v:.0}%")))
                    .collect();
                donut(
                    f,
                    area,
                    theme.panel("GPU", right, focused),
                    Donut {
                        segments: &segments,
                        center: format!("{:.0}%", t * 100.0),
                        sub: gpu_sub(g),
                        legend,
                    },
                    theme,
                );
            }
        },
    }
}

fn disk_donut(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let Some(m) = app.snapshot.disk.mounts.first() else {
        message(
            f,
            area,
            theme.panel("DISK", "", focused),
            "no disk detected",
            theme,
        );
        return;
    };
    let t = if m.total == 0 {
        0.0
    } else {
        (m.used as f64 / m.total as f64).clamp(0.0, 1.0)
    };
    let color = if t >= 0.9 {
        theme.hot
    } else {
        theme.palette[3]
    };
    let segments = [(t, color), (1.0 - t, theme.empty)];
    let legend = app
        .snapshot
        .disk
        .mounts
        .iter()
        .take(6)
        .map(|mm| {
            (
                color,
                format!(
                    "{} {}/{}",
                    mm.mount,
                    human_bytes(mm.used),
                    human_bytes(mm.total)
                ),
            )
        })
        .collect();
    donut(
        f,
        area,
        theme.panel(format!("DISK {}", m.mount), "", focused),
        Donut {
            segments: &segments,
            center: format!("{:.0}%", t * 100.0),
            sub: format!("{} / {}", human_bytes(m.used), human_bytes(m.total)),
            legend,
        },
        theme,
    );
}

fn core_bars(f: &mut Frame, area: Rect, per_core: &[f32], theme: &Theme) {
    let n = per_core.len() as u16;
    if n == 0 || area.width == 0 || area.height == 0 {
        return;
    }
    let step = (area.width / n).max(1);
    for (i, v) in per_core.iter().enumerate() {
        let x = area.x + i as u16 * step + step / 2;
        if x >= area.x + area.width {
            break;
        }
        let frac = (*v as f64 / 100.0).clamp(0.0, 1.0);
        for row in 0..area.height {
            let ch = vbar_symbol(frac, area.height, area.height - 1 - row);
            let color = if ch == ' ' {
                theme.empty
            } else {
                theme.level(frac)
            };
            f.buffer_mut()[(x, area.y + row)]
                .set_char(if ch == ' ' && row == area.height - 1 {
                    '▁'
                } else {
                    ch
                })
                .set_fg(color);
        }
    }
}

fn history(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let block = theme.panel(
        "HISTORY CPU · GPU",
        format!("{}s", app.history_secs),
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 4 || inner.width < 4 {
        return;
    }
    let bars_h = if inner.height >= 9 { 3 } else { 1 };
    let axis_h = if inner.height >= 12 { 2 } else { 0 };
    // Without a time axis, a line separates the core bars from the GPU chart.
    let bars_sep = u16::from(axis_h == 0);
    let [cpu_a, sep, gpu_a, time_a, sep2, bars] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(axis_h),
        Constraint::Length(bars_sep),
        Constraint::Length(bars_h),
    ])
    .areas(inner);
    let cpu_a = y_axis(f.buffer_mut(), cpu_a, &percent_labels(cpu_a.height), theme);
    let gpu_a = y_axis(f.buffer_mut(), gpu_a, &percent_labels(gpu_a.height), theme);
    if axis_h > 0 {
        x_axis(
            f.buffer_mut(),
            time_a,
            gpu_a,
            window_step(app, app.hist.cpu.capacity(), gpu_a.width),
            theme,
        );
    }
    let cpu_vals = fit_series(&app.hist.cpu.values(), app.hist.cpu.capacity(), cpu_a.width);
    f.render_widget(
        Area {
            values: &cpu_vals,
            invert: false,
            color: AreaColor::Level,
            theme,
        },
        cpu_a,
    );
    corner_label(
        f,
        cpu_a,
        &format!("cpu {:.0}%", app.snapshot.cpu.total),
        theme.fg,
    );
    for s in [sep, sep2] {
        f.render_widget(
            Paragraph::new("─".repeat(s.width as usize)).style(Style::new().fg(theme.border)),
            s,
        );
    }
    match (app.current_gpu(), app.hist.gpu.get(app.gpu_index)) {
        (Some(g), Some(h)) if g.util.ok().is_some() => {
            let vals = fit_series(&h.values(), h.capacity(), gpu_a.width);
            f.render_widget(
                Area {
                    values: &vals,
                    invert: false,
                    color: AreaColor::Fixed(theme.palette[0]),
                    theme,
                },
                gpu_a,
            );
            corner_label(
                f,
                gpu_a,
                &format!("gpu {:.0}%", g.util.ok().copied().unwrap_or(0.0)),
                theme.fg,
            );
        }
        _ => corner_label(f, gpu_a, "gpu: no usage data", theme.dim),
    }
    let bars = Rect::new(gpu_a.x, bars.y, gpu_a.width, bars.height);
    core_bars(f, bars, &app.snapshot.cpu.per_core, theme);
}

fn temps(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let block = theme.panel("TEMPERATURES", "°C", focused);
    let list = match &app.snapshot.temps {
        Avail::Unavailable(reason) => return message(f, area, block, reason, theme),
        Avail::Ok(list) => list,
    };
    let inner = block.inner(area);
    f.render_widget(block, area);
    let bar_w = inner.width.saturating_sub(9 + 8);
    for (i, s) in list.iter().take(inner.height as usize).enumerate() {
        let mut spans = vec![Span::styled(
            format!("{:<9}", truncate(&s.label, 8)),
            Style::new().fg(theme.fg),
        )];
        spans.extend(
            hbar_line(
                s.celsius as f64 / 100.0,
                bar_w,
                theme.temp(s.celsius),
                theme.empty,
            )
            .spans,
        );
        spans.push(Span::styled(
            format!("{:>6.0}°C", s.celsius),
            Style::new()
                .fg(theme.temp(s.celsius))
                .add_modifier(Modifier::BOLD),
        ));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
        );
    }
}

fn net(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let n = &app.snapshot.net;
    let block = theme.panel(format!("NETWORK · {}", n.iface_label), "", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width < 14 {
        return;
    }
    let axis_h = if inner.height >= 8 { 2 } else { 0 };
    let [body, time_a] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(axis_h)]).areas(inner);
    let chart = body;
    let half = chart.height / 2;
    let scale = app.hist.rx.max().max(app.hist.tx.max()).max(1.0);
    // Mirror: download upward and upload downward, with 0 in the middle.
    let top = format!("{:>4}", short_rate(scale));
    let mut marks = vec![
        (0, top.clone()),
        (chart.height - half - 1, format!("{:>4}", 0)),
    ];
    if half > 0 {
        marks.push((chart.height - 1, top));
    }
    let chart = y_axis(f.buffer_mut(), chart, &marks, theme);
    if axis_h > 0 {
        x_axis(
            f.buffer_mut(),
            time_a,
            chart,
            window_step(app, app.hist.rx.capacity(), chart.width),
            theme,
        );
    }
    let up = Rect::new(chart.x, chart.y, chart.width, chart.height - half);
    let down = Rect::new(chart.x, chart.y + up.height, chart.width, half);
    let fit = |h: &crate::history::History| -> Vec<f64> {
        fit_series(&h.values(), h.capacity(), chart.width)
            .iter()
            .map(|v| v / scale)
            .collect()
    };
    let (rx, tx) = (fit(&app.hist.rx), fit(&app.hist.tx));
    f.render_widget(
        Area {
            values: &rx,
            invert: false,
            color: AreaColor::Fixed(theme.rx),
            theme,
        },
        up,
    );
    f.render_widget(
        Area {
            values: &tx,
            invert: true,
            color: AreaColor::Fixed(theme.tx),
            theme,
        },
        down,
    );
    corner_label(f, up, &format!("↓ down {}", human_rate(n.rx_bps)), theme.rx);
    if down.height > 0 {
        let last = Rect::new(down.x, down.bottom() - 1, down.width, 1);
        corner_label(f, last, &format!("↑ up {}", human_rate(n.tx_bps)), theme.tx);
    }
}

fn disk_io(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let d = &app.snapshot.disk;
    let block = theme.panel("DISK I/O", truncate(&d.device, 20), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width < 12 {
        return;
    }
    let axis_h = if inner.height >= 8 { 2 } else { 0 };
    let [inner, time_a] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(axis_h)]).areas(inner);
    let sep = u16::from(inner.height >= 5);
    let half = (inner.height - sep) / 2;
    if sep > 0 {
        let y = inner.y + inner.height - half - 1;
        f.render_widget(
            Paragraph::new("─".repeat(inner.width as usize)).style(Style::new().fg(theme.border)),
            Rect::new(inner.x, y, inner.width, 1),
        );
    }
    let rows = [
        (
            Rect::new(inner.x, inner.y, inner.width, inner.height - half - sep),
            "read",
            d.read_bps,
            &app.hist.disk_r,
            theme.read,
        ),
        (
            Rect::new(inner.x, inner.y + inner.height - half, inner.width, half),
            "write",
            d.write_bps,
            &app.hist.disk_w,
            theme.write,
        ),
    ];
    for (rect, label, rate, hist, color) in rows {
        if rect.height == 0 {
            continue;
        }
        let scale = hist.max().max(1.0);
        let marks = [
            (0, format!("{:>4}", short_rate(scale))),
            (rect.height - 1, format!("{:>4}", 0)),
        ];
        let rect = y_axis(f.buffer_mut(), rect, &marks, theme);
        if axis_h > 0 && label == "write" {
            x_axis(
                f.buffer_mut(),
                time_a,
                rect,
                window_step(app, hist.capacity(), rect.width),
                theme,
            );
        }
        let vals: Vec<f64> = fit_series(&hist.values(), hist.capacity(), rect.width)
            .iter()
            .map(|v| v / scale)
            .collect();
        f.render_widget(
            Area {
                values: &vals,
                invert: false,
                color: AreaColor::Fixed(color),
                theme,
            },
            rect,
        );
        corner_label(f, rect, &format!("{label} {}", human_rate(rate)), color);
    }
}
