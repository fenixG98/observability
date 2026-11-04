//! Terminal interface: screen composition.

pub mod braille;
pub mod chrome;
pub mod compact;
pub mod dashboard;
pub mod processes;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::{App, Mode, Tab};
use theme::Theme;

/// Below this size, the dashboard switches to the compact view.
pub const COMPACT_MIN_WIDTH: u16 = 100;
pub const COMPACT_MIN_HEIGHT: u16 = 30;

pub fn draw(f: &mut Frame, app: &App, theme: &Theme) {
    let area = f.area();
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);
    chrome::tab_bar(f, top, app, theme);
    match app.tab {
        Tab::Dashboard if area.width < COMPACT_MIN_WIDTH || area.height < COMPACT_MIN_HEIGHT => {
            compact::render(f, body, app, theme)
        }
        Tab::Dashboard => dashboard::render(f, body, app, theme),
        Tab::Processes => processes::render(f, body, app, theme),
    }
    chrome::status_line(f, bottom, app, theme);
    match &app.mode {
        Mode::Help => chrome::help_overlay(f, body, theme),
        Mode::ConfirmKill { pid, name } => chrome::confirm_overlay(f, body, *pid, name, theme),
        _ => {}
    }
}
