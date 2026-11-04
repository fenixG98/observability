//! Tab bar, status / command line and overlay windows.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::app::{App, Mode, Tab};
use crate::command;
use crate::model::human_duration;
use crate::ui::theme::Theme;

/// Width of a line in columns, saturated to `u16`.
fn width_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

pub fn tab_bar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut spans = vec![Span::styled(
        " obstui ",
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    )];
    for (i, (tab, name)) in [(Tab::Dashboard, "Dashboard"), (Tab::Processes, "Processos")]
        .into_iter()
        .enumerate()
    {
        let label = format!(" {} {name} ", i + 1);
        spans.push(if app.tab == tab {
            Span::styled(
                label,
                Style::new()
                    .fg(theme.on_focus)
                    .bg(theme.focus)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(label, Style::new().fg(theme.dim))
        });
        spans.push(Span::raw(" "));
    }
    if app.prefix_armed {
        spans.push(Span::styled(
            " PREFIX ",
            Style::new().fg(theme.on_focus).bg(theme.warm),
        ));
    }
    let left = Line::from(spans);
    let left_width = width_u16(left.width());
    f.render_widget(Paragraph::new(left), area);

    let h = &app.snapshot.host;
    let right = format!(
        "{} · up {} · load {:.2} {:.2} {:.2} ",
        h.hostname,
        human_duration(h.uptime_secs),
        h.load[0],
        h.load[1],
        h.load[2]
    );
    // The host is only drawn if it doesn't overlap the left side.
    if left_width.saturating_add(width_u16(right.chars().count())) < area.width {
        f.render_widget(
            Paragraph::new(
                Line::from(Span::styled(right, Style::new().fg(theme.dim))).right_aligned(),
            ),
            area,
        );
    }
}

const HINTS: [(&str, &str); 7] = [
    ("1-2", "pestanya"),
    ("hjkl", "focus"),
    ("z", "zoom"),
    (":", "ordre"),
    ("/", "filtre"),
    ("?", "ajuda"),
    ("q", "sortir"),
];

fn input_line(prefix: &str, input: &str, theme: &Theme) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            prefix.to_string(),
            Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
        ),
        Span::styled(input.to_string(), Style::new().fg(theme.fg)),
        Span::styled(" ", Style::new().bg(theme.fg)),
    ]
}

pub fn status_line(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let line = match &app.mode {
        Mode::Command { input, error } => {
            let mut spans = input_line(":", input, theme);
            match error {
                Some(e) => spans.push(Span::styled(format!("  {e}"), Style::new().fg(theme.hot))),
                None => {
                    let candidates = command::complete(input);
                    if !candidates.is_empty() {
                        spans.push(Span::styled(
                            format!("  {}", candidates.join(" · ")),
                            Style::new().fg(theme.dim),
                        ));
                    }
                }
            }
            Line::from(spans)
        }
        Mode::Filter { input } => {
            let mut spans = input_line("/", input, theme);
            spans.push(Span::styled(
                "  enter aplica · esc esborra",
                Style::new().fg(theme.dim),
            ));
            Line::from(spans)
        }
        _ => match &app.status {
            Some(msg) => Line::from(Span::styled(
                format!(" {msg}"),
                Style::new().fg(theme.title),
            )),
            None => {
                let mut spans = vec![Span::raw(" ")];
                for (k, d) in HINTS {
                    spans.push(Span::styled(
                        k,
                        Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(format!(" {d}   "), Style::new().fg(theme.dim)));
                }
                Line::from(spans)
            }
        },
    };
    let left_width = width_u16(line.width());
    f.render_widget(Paragraph::new(line), area);
    if matches!(app.mode, Mode::Normal) {
        let right = format!("refresc {} ms ", app.refresh.as_millis());
        if left_width.saturating_add(width_u16(right.chars().count())) < area.width {
            f.render_widget(
                Paragraph::new(
                    Line::from(Span::styled(right, Style::new().fg(theme.dim))).right_aligned(),
                ),
                area,
            );
        }
    }
}

/// Rectangle of `w`×`h` centered within `area` (clipped if it doesn't fit).
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(area.width), h.min(area.height));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

const HELP: [(&str, &str); 18] = [
    ("1 / 2", "pestanya Dashboard / Processos"),
    ("n / p", "pestanya següent / anterior"),
    ("h j k l, fletxes", "moure el focus entre panells"),
    ("z", "zoom del panell actiu"),
    ("g", "següent GPU"),
    ("j / k (processos)", "moure la selecció"),
    ("x", "matar el procés seleccionat"),
    ("/", "filtrar processos"),
    (":", "mode d'ordres (Tab completa)"),
    ("?", "aquesta ajuda"),
    ("q, ctrl+c", "sortir"),
    ("", ""),
    (":dash  :proc", "canviar de pestanya"),
    (":sort cpu|mem|gpu|pid|name", "ordenar processos"),
    (":filter <text>", "filtrar (sense text = treure)"),
    (":kill <pid>", "matar un procés (demana confirmació)"),
    (":refresh 500ms", "interval de refresc"),
    (":gpu <n>  :net <iface|auto>", "triar GPU / interfície"),
];

pub fn help_overlay(f: &mut Frame, area: Rect, theme: &Theme) {
    let r = centered(area, 74, HELP.len() as u16 + 2);
    f.render_widget(Clear, r);
    let lines: Vec<Line> = HELP
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(
                    format!(" {k:<30}"),
                    Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
                ),
                Span::styled(d.to_string(), Style::new().fg(theme.fg)),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(theme.panel("AJUDA", "qualsevol tecla tanca", true)),
        r,
    );
}

pub fn confirm_overlay(f: &mut Frame, area: Rect, pid: u32, name: &str, theme: &Theme) {
    let r = centered(area, 52, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::from(Span::styled(
            format!(" Enviar SIGTERM a {name} ({pid})?"),
            Style::new().fg(theme.fg),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " s",
                Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" / enter: sí     ", Style::new().fg(theme.dim)),
            Span::styled(
                "qualsevol altra",
                Style::new().fg(theme.warm).add_modifier(Modifier::BOLD),
            ),
            Span::styled(": no", Style::new().fg(theme.dim)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(theme.panel("CONFIRMAR", "", true)),
        r,
    );
}
