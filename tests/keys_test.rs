use observability::config::Config;
use observability::keys::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn k(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}
fn code(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}
const DASH: KeyContext = KeyContext {
    processes_tab: false,
};
const PROC: KeyContext = KeyContext {
    processes_tab: true,
};

fn disp(prefix: bool) -> Dispatcher {
    let mut c = Config::default();
    c.prefix.enabled = prefix;
    Dispatcher::new(KeyMap::from_config(&c).unwrap())
}

#[test]
fn parse_key_variants() {
    assert_eq!(
        parse_key("q").unwrap(),
        KeySpec {
            code: KeyCode::Char('q'),
            ctrl: false
        }
    );
    assert_eq!(
        parse_key("ctrl+b").unwrap(),
        KeySpec {
            code: KeyCode::Char('b'),
            ctrl: true
        }
    );
    assert_eq!(
        parse_key("CTRL+B").unwrap(),
        KeySpec {
            code: KeyCode::Char('b'),
            ctrl: true
        }
    );
    assert_eq!(parse_key("esc").unwrap().code, KeyCode::Esc);
    assert_eq!(parse_key("left").unwrap().code, KeyCode::Left);
    assert_eq!(parse_key("space").unwrap().code, KeyCode::Char(' '));
    assert!(parse_key("ab").is_err());
    assert!(parse_key("").is_err());
}

#[test]
fn direct_keys_without_prefix() {
    let mut d = disp(false);
    assert_eq!(d.handle(&k('q'), DASH), Some(Action::Quit));
    assert_eq!(d.handle(&k('1'), DASH), Some(Action::Tab(0)));
    assert_eq!(d.handle(&k('2'), DASH), Some(Action::Tab(1)));
    assert_eq!(d.handle(&k('n'), DASH), Some(Action::NextTab));
    assert_eq!(d.handle(&k('h'), DASH), Some(Action::Focus(Dir::Left)));
    assert_eq!(
        d.handle(&code(KeyCode::Left), DASH),
        Some(Action::Focus(Dir::Left))
    );
    assert_eq!(d.handle(&k('j'), DASH), Some(Action::Focus(Dir::Down)));
    assert_eq!(d.handle(&k('z'), DASH), Some(Action::Zoom));
    assert_eq!(d.handle(&k('g'), DASH), Some(Action::GpuNext));
    assert_eq!(d.handle(&k(':'), DASH), Some(Action::OpenCommand));
    assert_eq!(d.handle(&k('/'), DASH), Some(Action::OpenFilter));
    assert_eq!(d.handle(&k('?'), DASH), Some(Action::Help));
    assert_eq!(d.handle(&k('x'), DASH), Some(Action::Kill));
    assert_eq!(d.handle(&k('w'), DASH), None);
}

#[test]
fn processes_tab_up_down_move_rows() {
    let mut d = disp(false);
    assert_eq!(d.handle(&k('j'), PROC), Some(Action::RowDown));
    assert_eq!(d.handle(&k('k'), PROC), Some(Action::RowUp));
    assert_eq!(d.handle(&code(KeyCode::Down), PROC), Some(Action::RowDown));
    assert_eq!(d.handle(&k('h'), PROC), Some(Action::Focus(Dir::Left)));
}

#[test]
fn prefix_required_for_navigation() {
    let mut d = disp(true);
    assert_eq!(d.handle(&k('1'), DASH), None);
    assert_eq!(d.handle(&ctrl('b'), DASH), None);
    assert!(d.prefix_armed());
    assert_eq!(d.handle(&k('1'), DASH), Some(Action::Tab(0)));
    assert!(!d.prefix_armed());
    assert_eq!(d.handle(&k('l'), DASH), None);
}

#[test]
fn prefix_then_non_navigation_cancels() {
    let mut d = disp(true);
    d.handle(&ctrl('b'), DASH);
    assert_eq!(d.handle(&k('q'), DASH), None);
    assert!(!d.prefix_armed());
    assert_eq!(d.handle(&k('q'), DASH), Some(Action::Quit));
}

#[test]
fn direct_actions_ignore_prefix() {
    let mut d = disp(true);
    assert_eq!(d.handle(&k(':'), DASH), Some(Action::OpenCommand));
    assert_eq!(d.handle(&k('/'), DASH), Some(Action::OpenFilter));
    assert_eq!(d.handle(&k('j'), PROC), Some(Action::RowDown));
    d.handle(&ctrl('b'), PROC);
    assert_eq!(d.handle(&k('j'), PROC), Some(Action::Focus(Dir::Down)));
}

#[test]
fn ctrl_c_always_quits() {
    let mut d = disp(true);
    d.handle(&ctrl('b'), DASH);
    assert_eq!(d.handle(&ctrl('c'), DASH), Some(Action::Quit));
}

#[test]
fn duplicate_or_invalid_keys_are_errors() {
    let mut c = Config::default();
    c.keys.zoom = "q".into();
    assert!(KeyMap::from_config(&c).err().unwrap().contains("twice"));
    let mut c = Config::default();
    c.prefix.enabled = true;
    c.prefix.key = "ctrl+".into();
    assert!(KeyMap::from_config(&c).is_err());
}
