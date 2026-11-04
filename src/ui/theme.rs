//! Colors (truecolor or 256), thresholds and common panel block.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub fg: Color,
    pub dim: Color,
    pub border: Color,
    pub focus: Color,
    pub title: Color,
    pub accent: Color,
    pub ok: Color,
    pub warm: Color,
    pub hot: Color,
    pub rx: Color,
    pub tx: Color,
    pub read: Color,
    pub write: Color,
    pub empty: Color,
    pub selection: Color,
    /// Text on focus background (active tab).
    pub on_focus: Color,
    pub palette: [Color; 6],
    pub temp_warm: f32,
    pub temp_hot: f32,
}

pub fn detect_truecolor(colorterm: Option<&str>) -> bool {
    colorterm
        .is_some_and(|v| v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit"))
}

impl Theme {
    pub fn new(truecolor: bool, temp_warm: f32, temp_hot: f32) -> Theme {
        let c = |rgb: (u8, u8, u8), idx: u8| {
            if truecolor {
                Color::Rgb(rgb.0, rgb.1, rgb.2)
            } else {
                Color::Indexed(idx)
            }
        };
        Theme {
            fg: c((201, 209, 217), 252),
            dim: c((110, 118, 129), 243),
            border: c((59, 66, 82), 238),
            focus: c((122, 162, 247), 111),
            title: c((224, 175, 104), 179),
            accent: c((187, 154, 247), 141),
            ok: c((158, 206, 106), 149),
            warm: c((224, 175, 104), 179),
            hot: c((247, 118, 142), 211),
            rx: c((125, 207, 255), 117),
            tx: c((187, 154, 247), 141),
            read: c((158, 206, 106), 149),
            write: c((247, 118, 142), 211),
            empty: c((59, 66, 82), 238),
            selection: c((40, 52, 87), 236),
            on_focus: c((15, 17, 23), 234),
            palette: [
                c((122, 162, 247), 111),
                c((115, 218, 202), 79),
                c((187, 154, 247), 141),
                c((255, 158, 100), 215),
                c((125, 207, 255), 117),
                c((158, 206, 106), 149),
            ],
            temp_warm,
            temp_hot,
        }
    }

    /// Color for a usage fraction 0..=1.
    pub fn level(&self, fraction: f64) -> Color {
        if fraction < 0.5 {
            self.ok
        } else if fraction < 0.8 {
            self.warm
        } else {
            self.hot
        }
    }

    pub fn temp(&self, celsius: f32) -> Color {
        if celsius < self.temp_warm {
            self.ok
        } else if celsius < self.temp_hot {
            self.warm
        } else {
            self.hot
        }
    }

    /// Rounded block: title on the left, dimmed text on the right, blue border if focused.
    pub fn panel(
        &self,
        title: impl Into<String>,
        right: impl Into<String>,
        focused: bool,
    ) -> Block<'static> {
        let border = if focused { self.focus } else { self.border };
        let title_color = if focused { self.focus } else { self.title };
        let right: String = right.into();
        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(border))
            .title(
                Line::from(format!(" {} ", title.into()))
                    .style(Style::new().fg(title_color).add_modifier(Modifier::BOLD)),
            );
        if !right.is_empty() {
            block = block.title_top(
                Line::from(format!(" {right} "))
                    .style(Style::new().fg(self.dim))
                    .right_aligned(),
            );
        }
        block
    }
}

/// Cuts `s` to `max` characters, ending with "…" if cut.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
