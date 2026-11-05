//! Configurable keys, actions and dispatcher with optional tmux-style prefix.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub code: KeyCode,
    pub ctrl: bool,
}

impl KeySpec {
    pub fn from_event(ev: &KeyEvent) -> KeySpec {
        let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        let code = match ev.code {
            KeyCode::Char(c) if ctrl => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        KeySpec { code, ctrl }
    }
}

/// "q", "ctrl+b", "esc", "enter", "tab", "backspace", "left", "right", "up", "down", "space".
pub fn parse_key(s: &str) -> Result<KeySpec, String> {
    let t = s.trim();
    let (ctrl, rest) = match t.get(..5) {
        Some(p) if p.eq_ignore_ascii_case("ctrl+") => (true, &t[5..]),
        _ => (false, t),
    };
    let code = match rest.to_ascii_lowercase().as_str() {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "space" => KeyCode::Char(' '),
        _ => {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(if ctrl { c.to_ascii_lowercase() } else { c }),
                _ => return Err(format!("unknown key: \"{s}\"")),
            }
        }
    };
    Ok(KeySpec { code, ctrl })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Tab(usize),
    NextTab,
    PrevTab,
    Focus(Dir),
    Zoom,
    GpuNext,
    OpenCommand,
    OpenFilter,
    Help,
    Quit,
    Kill,
    RowUp,
    RowDown,
}

impl Action {
    /// Actions that require the prefix when it is enabled.
    pub fn is_navigation(self) -> bool {
        matches!(
            self,
            Action::Tab(_)
                | Action::NextTab
                | Action::PrevTab
                | Action::Focus(_)
                | Action::Zoom
                | Action::GpuNext
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyContext {
    pub processes_tab: bool,
}

#[derive(Debug, Clone)]
pub struct KeyMap {
    bindings: Vec<(KeySpec, Action)>,
    prefix: Option<KeySpec>,
}

impl KeyMap {
    pub fn from_config(cfg: &Config) -> Result<KeyMap, String> {
        let k = &cfg.keys;
        let pairs = [
            (&k.tab_dashboard, Action::Tab(0)),
            (&k.tab_processes, Action::Tab(1)),
            (&k.next_tab, Action::NextTab),
            (&k.prev_tab, Action::PrevTab),
            (&k.left, Action::Focus(Dir::Left)),
            (&k.down, Action::Focus(Dir::Down)),
            (&k.up, Action::Focus(Dir::Up)),
            (&k.right, Action::Focus(Dir::Right)),
            (&k.zoom, Action::Zoom),
            (&k.gpu_next, Action::GpuNext),
            (&k.command, Action::OpenCommand),
            (&k.filter, Action::OpenFilter),
            (&k.help, Action::Help),
            (&k.quit, Action::Quit),
            (&k.kill, Action::Kill),
        ];
        let mut bindings: Vec<(KeySpec, Action)> = Vec::new();
        for (text, action) in pairs {
            let spec = parse_key(text)?;
            if bindings.iter().any(|(s, _)| *s == spec) {
                return Err(format!("key bound twice in the config: \"{text}\""));
            }
            bindings.push((spec, action));
        }
        // Arrows always move the focus, in addition to the configured keys.
        for (code, dir) in [
            (KeyCode::Left, Dir::Left),
            (KeyCode::Down, Dir::Down),
            (KeyCode::Up, Dir::Up),
            (KeyCode::Right, Dir::Right),
        ] {
            bindings.push((KeySpec { code, ctrl: false }, Action::Focus(dir)));
        }
        let prefix = if cfg.prefix.enabled {
            Some(parse_key(&cfg.prefix.key)?)
        } else {
            None
        };
        Ok(KeyMap { bindings, prefix })
    }

    fn lookup(&self, key: KeySpec) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(s, _)| *s == key)
            .map(|(_, a)| *a)
    }
}

#[derive(Debug, Clone)]
pub struct Dispatcher {
    map: KeyMap,
    armed: bool,
}

impl Dispatcher {
    pub fn new(map: KeyMap) -> Dispatcher {
        Dispatcher { map, armed: false }
    }

    pub fn prefix_armed(&self) -> bool {
        self.armed
    }

    /// Translates a key (in normal mode) into an action.
    pub fn handle(&mut self, ev: &KeyEvent, ctx: KeyContext) -> Option<Action> {
        let key = KeySpec::from_event(ev);
        if key
            == (KeySpec {
                code: KeyCode::Char('c'),
                ctrl: true,
            })
        {
            self.armed = false;
            return Some(Action::Quit);
        }
        if let Some(prefix) = self.map.prefix {
            if self.armed {
                self.armed = false;
                return self.map.lookup(key).filter(|a| a.is_navigation());
            }
            if key == prefix {
                self.armed = true;
                return None;
            }
        }
        let action = self.map.lookup(key)?;
        if ctx.processes_tab {
            match action {
                Action::Focus(Dir::Up) => return Some(Action::RowUp),
                Action::Focus(Dir::Down) => return Some(Action::RowDown),
                _ => {}
            }
        }
        if self.map.prefix.is_some() && action.is_navigation() {
            return None;
        }
        Some(action)
    }
}
