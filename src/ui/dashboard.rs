//! Tab 1: layout "B · Pies on top".

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::app::{App, Panel};
use crate::model::{Avail, GpuReading, MemKind, human_bytes, human_rate};
use crate::ui::braille::{Area, AreaColor, Pie, hbar_line, vbar_symbol};
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
fn corner_label(f: &mut Frame, area: Rect, text: &str, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let text = truncate(text, area.width as usize);
    f.buffer_mut().set_string(
        area.x,
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
    let [pie_area, text] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
    let pie_w = (pie_area.height * 2).min(pie_area.width);
    let pie_area = if !d.legend.is_empty() && pie_area.width >= pie_w + 26 {
        let [p, legend] = Layout::horizontal([Constraint::Length(pie_w + 2), Constraint::Fill(1)])
            .areas(pie_area);
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
        p
    } else {
        pie_area
    };
    f.render_widget(
        Pie {
            segments: d.segments,
            hole: 0.6,
            label: Some((&d.center, theme.fg)),
        },
        pie_area,
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(d.sub, Style::new().fg(theme.dim))).centered()),
        text,
    );
}

fn cpu_donut(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let cpu = &app.snapshot.cpu;
    let t = (cpu.total as f64 / 100.0).clamp(0.0, 1.0);
    let sub = if cpu.freq_mhz > 0 {
        format!(
            "{} fils · {:.1} GHz",
            cpu.per_core.len(),
            cpu.freq_mhz as f64 / 1000.0
        )
    } else {
        format!("{} fils", cpu.per_core.len())
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
        theme.panel("MEMÒRIA", right, focused),
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
            "no s'ha detectat cap GPU",
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
            theme.panel("DISC", "", focused),
            "cap disc detectat",
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
        theme.panel(format!("DISC {}", m.mount), "", focused),
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
        "HISTORIAL CPU · GPU",
        format!("{}s", app.history_secs),
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 4 || inner.width < 4 {
        return;
    }
    let bars_h = if inner.height >= 9 { 3 } else { 1 };
    let [cpu_a, sep, gpu_a, bars] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(bars_h),
    ])
    .areas(inner);
    let cpu_vals = app.hist.cpu.values();
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
    f.render_widget(
        Paragraph::new("─".repeat(sep.width as usize)).style(Style::new().fg(theme.border)),
        sep,
    );
    match (app.current_gpu(), app.hist.gpu.get(app.gpu_index)) {
        (Some(g), Some(h)) if g.util.ok().is_some() => {
            let vals = h.values();
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
        _ => corner_label(f, gpu_a, "gpu: sense dades d'ús", theme.dim),
    }
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
    let block = theme.panel(
        format!("XARXA  {}", n.iface_label),
        format!("↓ {}  ↑ {}", human_rate(n.rx_bps), human_rate(n.tx_bps)),
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width < 14 {
        return;
    }
    let [labels, chart] =
        Layout::horizontal([Constraint::Length(13), Constraint::Fill(1)]).areas(inner);
    let half = chart.height / 2;
    let up = Rect::new(chart.x, chart.y, chart.width, chart.height - half);
    let down = Rect::new(chart.x, chart.y + up.height, chart.width, half);
    let scale = app.hist.rx.max().max(app.hist.tx.max()).max(1.0);
    let rx: Vec<f64> = app.hist.rx.values().iter().map(|v| v / scale).collect();
    let tx: Vec<f64> = app.hist.tx.values().iter().map(|v| v / scale).collect();
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
    let bold = |c: Color| Style::new().fg(c).add_modifier(Modifier::BOLD);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("↓ {}", human_rate(n.rx_bps)),
            bold(theme.rx),
        ))),
        Rect::new(labels.x, labels.y, labels.width, 1),
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("↑ {}", human_rate(n.tx_bps)),
            bold(theme.tx),
        ))),
        Rect::new(labels.x, labels.y + labels.height - 1, labels.width, 1),
    );
    if labels.height >= 4 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("màx {}", human_rate(scale as u64)),
                Style::new().fg(theme.dim),
            ))),
            Rect::new(labels.x, labels.y + 1, labels.width, 1),
        );
    }
}

fn disk_io(f: &mut Frame, area: Rect, app: &App, theme: &Theme, focused: bool) {
    let d = &app.snapshot.disk;
    let block = theme.panel("DISC I/O", truncate(&d.device, 20), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width < 12 {
        return;
    }
    let half = inner.height / 2;
    let rows = [
        (
            Rect::new(inner.x, inner.y, inner.width, inner.height - half),
            "llegir",
            d.read_bps,
            &app.hist.disk_r,
            theme.read,
        ),
        (
            Rect::new(inner.x, inner.y + inner.height - half, inner.width, half),
            "escriure",
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
        let vals: Vec<f64> = hist.values().iter().map(|v| v / scale).collect();
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
