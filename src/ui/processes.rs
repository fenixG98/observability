//! Tab 2: process table, "who uses the CPU" pie and detail of the selected one.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{App, Panel};
use crate::model::{human_bytes, human_duration};
use crate::ui::braille::{Area, AreaColor, Pie, hbar_line};
use crate::ui::theme::{Theme, truncate};

pub fn render(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    if app.zoomed {
        match app.focus {
            Panel::ProcPie => pie(f, area, app, theme),
            Panel::ProcDetail => detail(f, area, app, theme),
            _ => table(f, area, app, theme),
        }
        return;
    }
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(66), Constraint::Fill(1)]).areas(area);
    let [pie_area, detail_area] =
        Layout::vertical([Constraint::Percentage(55), Constraint::Fill(1)]).areas(right);
    table(f, left, app, theme);
    pie(f, pie_area, app, theme);
    detail(f, detail_area, app, theme);
}

fn note(
    f: &mut Frame,
    area: Rect,
    block: ratatui::widgets::Block<'static>,
    text: &str,
    theme: &Theme,
) {
    f.render_widget(
        Paragraph::new(text.to_string())
            .style(Style::new().fg(theme.dim))
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

fn table(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let procs = app.visible_procs();
    let right = format!(
        "{} · ordre: {} · filtre: {}",
        procs.len(),
        app.sort.label(),
        app.filter.as_deref().unwrap_or("–")
    );
    let block = theme
        .panel("PROCESSOS", right, app.focus == Panel::ProcTable)
        .title_bottom(
            Line::from(" j/k moure · x matar · / filtrar · :sort ")
                .style(Style::new().fg(theme.dim)),
        );
    if app.snapshot.procs.is_none() {
        return note(f, area, block, "carregant processos…", theme);
    }
    let header = Row::new(["    PID", "NOM", "USUARI", "  CPU%", "", "   MEM", " GPU%"])
        .style(Style::new().fg(theme.title).add_modifier(Modifier::BOLD));
    let rows = procs.iter().map(|p| {
        let frac = (p.cpu as f64 / 100.0).clamp(0.0, 1.0);
        Row::new(vec![
            Cell::from(format!("{:>7}", p.pid)),
            Cell::from(p.name.clone()),
            Cell::from(truncate(&p.user, 9)),
            Cell::from(format!("{:>6.1}", p.cpu)),
            Cell::from(hbar_line(frac, 10, theme.level(frac), theme.empty)),
            Cell::from(format!("{:>6}", human_bytes(p.mem))),
            Cell::from(
                p.gpu
                    .map_or_else(|| "    –".to_string(), |g| format!("{g:>4.0}%")),
            ),
        ])
        .style(Style::new().fg(theme.fg))
    });
    let widths = [
        Constraint::Length(7),
        Constraint::Fill(1),
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(5),
    ];
    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(
        Table::new(rows, widths)
            .header(header)
            .block(block)
            .column_spacing(1)
            .row_highlight_style(
                Style::new()
                    .bg(theme.selection)
                    .add_modifier(Modifier::BOLD),
            ),
        area,
        &mut state,
    );
}

fn pie(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = theme.panel("QUI GASTA LA CPU", "top 4", app.focus == Panel::ProcPie);
    let shares = app.cpu_share(4);
    if shares.is_empty() {
        return note(f, area, block, "sense ús de CPU per mostrar", theme);
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    let colors: Vec<_> = shares
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            if name == "altres" {
                theme.empty
            } else {
                theme.palette[i % theme.palette.len()]
            }
        })
        .collect();
    let segments: Vec<(f64, _)> = shares
        .iter()
        .zip(&colors)
        .map(|((_, v), c)| (*v, *c))
        .collect();
    let legend_h = (shares.len() as u16).min(inner.height.saturating_sub(1));
    let [pie_area, legend] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(legend_h)]).areas(inner);
    f.render_widget(
        Pie {
            segments: &segments,
            hole: 0.0,
            label: None,
            aspect: theme.cell_aspect,
        },
        pie_area,
    );
    let lines: Vec<Line> = shares
        .iter()
        .zip(&colors)
        .map(|((name, v), c)| {
            Line::from(vec![
                Span::styled(" ■ ", Style::new().fg(*c)),
                Span::styled(
                    format!("{:<16}", truncate(name, 15)),
                    Style::new().fg(theme.fg),
                ),
                Span::styled(format!("{:>4.0}%", v * 100.0), Style::new().fg(theme.dim)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), legend);
}

fn detail(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let focused = app.focus == Panel::ProcDetail;
    let Some(p) = app.selected_proc() else {
        return note(
            f,
            area,
            theme.panel("DETALL", "", focused),
            "cap procés seleccionat",
            theme,
        );
    };
    let block = theme.panel(
        format!("{} · {}", truncate(&p.name, 20), p.pid),
        "",
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 || inner.width < 6 {
        return;
    }
    let chart_h = if inner.height >= 8 { 2 } else { 0 };
    let [chart, text] =
        Layout::vertical([Constraint::Length(chart_h), Constraint::Fill(1)]).areas(inner);
    if chart_h > 0
        && let Some(h) = app.hist.proc_cpu.get(&p.pid)
    {
        let scale = h.max().max(100.0);
        let vals: Vec<f64> = h.values().iter().map(|v| v / scale).collect();
        f.render_widget(
            Area {
                values: &vals,
                invert: false,
                color: AreaColor::Level,
                theme,
            },
            chart,
        );
    }
    let dim = Style::new().fg(theme.dim);
    let val = Style::new().fg(theme.fg);
    let gpu = p
        .gpu
        .map_or_else(|| "–".to_string(), |g| format!("{g:.0}%"));
    // Linux doesn't always expose the thread count: the line is still shown with "–".
    let threads = p.threads.map_or_else(|| "–".to_string(), |t| t.to_string());
    let lines = vec![
        Line::from(vec![
            Span::styled("cpu ", dim),
            Span::styled(format!("{:.1}%", p.cpu), val),
            Span::styled("   mem ", dim),
            Span::styled(human_bytes(p.mem), val),
            Span::styled("   gpu ", dim),
            Span::styled(gpu, val),
        ]),
        Line::from(vec![
            Span::styled("usuari ", dim),
            Span::styled(p.user.clone(), val),
        ]),
        Line::from(vec![
            Span::styled("en marxa ", dim),
            Span::styled(
                format!(
                    "fa {}",
                    human_duration(app.snapshot.taken_at.saturating_sub(p.start_time))
                ),
                val,
            ),
        ]),
        Line::from(vec![Span::styled("fils ", dim), Span::styled(threads, val)]),
        Line::from(Span::styled(truncate(&p.exe, inner.width as usize), dim)),
        Line::from(vec![
            Span::styled(
                "x",
                Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" matar", dim),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), text);
}
