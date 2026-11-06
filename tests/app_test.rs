mod common;

use common::*;
use guaitop::app::*;
use guaitop::command::{Command, SortKey};
use guaitop::config::Config;
use guaitop::keys::{Action, Dir, Dispatcher, KeyMap};
use guaitop::model::Avail;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

fn disp() -> Dispatcher {
    Dispatcher::new(KeyMap::from_config(&Config::default()).unwrap())
}
fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}
fn typ(app: &mut App, d: &mut Dispatcher, text: &str) -> Vec<Effect> {
    let mut fx = vec![];
    for c in text.chars() {
        fx.extend(app.on_key(&key(KeyCode::Char(c)), d));
    }
    fx
}
fn names(app: &App) -> Vec<String> {
    app.visible_procs().iter().map(|p| p.name.clone()).collect()
}

#[test]
fn new_app_defaults() {
    let app = App::new(&Config::default());
    assert_eq!(app.tab, Tab::Dashboard);
    assert_eq!(app.focus, Panel::CpuDonut);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.history_len(), 60);
    assert_eq!(app.sort, SortKey::Cpu);
}

#[test]
fn switching_tabs_toggles_process_collection() {
    let mut app = sample_app(1);
    assert_eq!(
        app.apply_action(Action::Tab(1)),
        vec![Effect::SetProcessesVisible(true)]
    );
    assert_eq!(app.focus, Panel::ProcTable);
    assert!(app.apply_action(Action::Tab(1)).is_empty());
    assert_eq!(
        app.apply_action(Action::NextTab),
        vec![Effect::SetProcessesVisible(false)]
    );
    assert_eq!(app.tab, Tab::Dashboard);
}

#[test]
fn focus_moves_on_the_dashboard_grid() {
    let mut app = sample_app(1);
    let mut go = |d| {
        app.apply_action(Action::Focus(d));
        app.focus
    };
    assert_eq!(go(Dir::Left), Panel::CpuDonut);
    assert_eq!(go(Dir::Right), Panel::MemDonut);
    assert_eq!(go(Dir::Down), Panel::History);
    assert_eq!(go(Dir::Right), Panel::Temps);
    assert_eq!(go(Dir::Down), Panel::DiskIo);
    assert_eq!(go(Dir::Left), Panel::Net);
    assert_eq!(go(Dir::Up), Panel::History);
    assert_eq!(go(Dir::Up), Panel::CpuDonut);
    assert_eq!(neighbor(Panel::ProcTable, Dir::Right), Some(Panel::ProcPie));
    assert_eq!(neighbor(Panel::ProcPie, Dir::Down), Some(Panel::ProcDetail));
}

#[test]
fn zoom_freezes_focus() {
    let mut app = sample_app(1);
    app.apply_action(Action::Zoom);
    assert!(app.zoomed);
    app.apply_action(Action::Focus(Dir::Right));
    assert_eq!(app.focus, Panel::CpuDonut);
    app.apply_action(Action::Zoom);
    assert!(!app.zoomed);
}

#[test]
fn sorting_and_filtering_processes() {
    let mut app = sample_app(1);
    assert_eq!(
        names(&app),
        ["firefox", "rust-analyzer", "ghostty", "kwin_x11", "systemd"]
    );
    app.apply_command(Command::Sort(SortKey::Mem));
    assert_eq!(names(&app)[0], "rust-analyzer");
    app.apply_command(Command::Sort(SortKey::Gpu));
    assert_eq!(
        names(&app),
        ["kwin_x11", "firefox", "ghostty", "systemd", "rust-analyzer"]
    );
    app.apply_command(Command::Sort(SortKey::Name));
    assert_eq!(
        names(&app),
        ["firefox", "ghostty", "kwin_x11", "rust-analyzer", "systemd"]
    );
    app.apply_command(Command::Sort(SortKey::Pid));
    assert_eq!(names(&app)[0], "systemd");
    app.apply_command(Command::Filter(Some("FIRE".into())));
    assert_eq!(names(&app), ["firefox"]);
    app.apply_command(Command::Filter(Some("44".into())));
    assert_eq!(names(&app), ["firefox"]);
    app.apply_command(Command::Filter(None));
    assert_eq!(names(&app).len(), 5);
}

#[test]
fn row_navigation_is_clamped() {
    let mut app = sample_app(1);
    app.apply_action(Action::Tab(1));
    app.apply_action(Action::RowUp);
    assert_eq!(app.selected, 0);
    for _ in 0..10 {
        app.apply_action(Action::RowDown);
    }
    assert_eq!(app.selected, 4);
    assert_eq!(app.selected_proc().unwrap().name, "systemd");
}

#[test]
fn command_mode_flow() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.on_key(&key(KeyCode::Char(':')), &mut d);
    assert!(matches!(app.mode, Mode::Command { .. }));
    typ(&mut app, &mut d, "sort mem");
    app.on_key(&key(KeyCode::Enter), &mut d);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.sort, SortKey::Mem);
}

#[test]
fn command_errors_keep_the_input() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.on_key(&key(KeyCode::Char(':')), &mut d);
    typ(&mut app, &mut d, "foo");
    app.on_key(&key(KeyCode::Enter), &mut d);
    match &app.mode {
        Mode::Command { input, error } => {
            assert_eq!(input, "foo");
            assert!(error.as_ref().unwrap().contains("unknown"));
        }
        m => panic!("mode inesperat: {m:?}"),
    }
    app.on_key(&key(KeyCode::Esc), &mut d);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn tab_completes_and_backspace_exits_when_empty() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.on_key(&key(KeyCode::Char(':')), &mut d);
    typ(&mut app, &mut d, "so");
    app.on_key(&key(KeyCode::Tab), &mut d);
    assert_eq!(
        app.mode,
        Mode::Command {
            input: "sort ".into(),
            error: None
        }
    );
    for _ in 0..5 {
        app.on_key(&key(KeyCode::Backspace), &mut d);
    }
    assert!(matches!(app.mode, Mode::Command { .. }));
    app.on_key(&key(KeyCode::Backspace), &mut d);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn filter_mode_is_live_and_esc_clears() {
    let (mut app, mut d) = (sample_app(1), disp());
    let fx = app.on_key(&key(KeyCode::Char('/')), &mut d);
    assert_eq!(fx, vec![Effect::SetProcessesVisible(true)]);
    assert_eq!(app.tab, Tab::Processes);
    typ(&mut app, &mut d, "gho");
    assert_eq!(app.filter.as_deref(), Some("gho"));
    assert_eq!(names(&app), ["ghostty"]);
    app.on_key(&key(KeyCode::Esc), &mut d);
    assert_eq!(app.filter, None);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn kill_requires_confirmation() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.apply_action(Action::Tab(1));
    app.on_key(&key(KeyCode::Char('x')), &mut d);
    assert_eq!(
        app.mode,
        Mode::ConfirmKill {
            pid: 4412,
            name: "firefox".into()
        }
    );
    assert_eq!(
        app.on_key(&key(KeyCode::Char('s')), &mut d),
        vec![Effect::Kill(4412)]
    );
    app.on_key(&key(KeyCode::Char('x')), &mut d);
    assert!(app.on_key(&key(KeyCode::Char('n')), &mut d).is_empty());
    assert_eq!(app.status.as_deref(), Some("cancelled"));
    app.apply_command(Command::Kill(1));
    assert_eq!(
        app.mode,
        Mode::ConfirmKill {
            pid: 1,
            name: "systemd".into()
        }
    );
}

#[test]
fn kill_key_does_nothing_on_dashboard() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.on_key(&key(KeyCode::Char('x')), &mut d);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn refresh_command_resizes_histories() {
    let mut app = sample_app(1);
    let fx = app.apply_command(Command::Refresh(Duration::from_millis(500)));
    assert_eq!(fx, vec![Effect::SetRefresh(Duration::from_millis(500))]);
    assert_eq!(app.history_len(), 120);
    assert_eq!(app.hist.cpu.capacity(), 120);
}

#[test]
fn gpu_and_net_commands() {
    let mut app = sample_app(1);
    app.apply_command(Command::Gpu(3));
    assert!(app.status.as_deref().unwrap().contains("no GPU 4"));
    assert_eq!(app.gpu_index, 0);
    assert_eq!(
        app.apply_command(Command::Net(Some("eth0".into()))),
        vec![Effect::SetNetIface(Some("eth0".into()))]
    );
}

#[test]
fn quit_from_key_and_command() {
    let (mut app, mut d) = (sample_app(1), disp());
    assert_eq!(
        app.on_key(&key(KeyCode::Char('q')), &mut d),
        vec![Effect::Quit]
    );
    app.on_key(&key(KeyCode::Char(':')), &mut d);
    app.on_key(&key(KeyCode::Char('q')), &mut d);
    assert_eq!(app.on_key(&key(KeyCode::Enter), &mut d), vec![Effect::Quit]);
}

#[test]
fn help_closes_with_any_key() {
    let (mut app, mut d) = (sample_app(1), disp());
    app.on_key(&key(KeyCode::Char('?')), &mut d);
    assert_eq!(app.mode, Mode::Help);
    app.on_key(&key(KeyCode::Char('w')), &mut d);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn cpu_share_has_top_n_plus_others() {
    let app = sample_app(1);
    let share = app.cpu_share(2);
    assert_eq!(share.len(), 3);
    assert_eq!(share[0].0, "firefox");
    assert_eq!(share[2].0, "other");
    assert!((share.iter().map(|s| s.1).sum::<f64>() - 1.0).abs() < 1e-9);
    assert!((share[0].1 - 12.8 / 28.1).abs() < 1e-3);
}

#[test]
fn snapshots_feed_histories() {
    let mut app = sample_app(3);
    assert_eq!(app.hist.cpu.len(), 3);
    assert_eq!(app.hist.gpu.len(), 1);
    assert_eq!(app.hist.rx.len(), 3);
    app.apply_action(Action::Tab(1));
    app.on_snapshot(sample_snapshot());
    assert_eq!(
        app.hist.proc_cpu.keys().copied().collect::<Vec<_>>(),
        vec![4412]
    );
    let mut s = sample_snapshot();
    s.gpus = Avail::Unavailable("cap".into());
    app.gpu_index = 0;
    app.on_snapshot(s);
    assert_eq!(app.gpu_count(), 0);
    assert!(app.hist.gpu.is_empty());
}
