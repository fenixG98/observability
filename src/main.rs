use std::io;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use guaitop::app::{App, Effect};
use guaitop::collect::system::SystemSource;
use guaitop::collect::{self, Control, Source};
use guaitop::config::{self, Config};
use guaitop::keys::{Dispatcher, KeyMap};
use guaitop::model::Snapshot;
use guaitop::ui::{
    self,
    pixels::Graphics,
    theme::{Theme, cell_aspect, detect_truecolor},
};
use guaitop::{gpu, probe};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn help_text() -> String {
    let path = config::default_path().map_or_else(
        || "(not available)".to_string(),
        |p| p.display().to_string(),
    );
    format!(
        "guaitop — terminal resource monitor

Usage: guaitop [--probe | --version | --help]

  (no arguments)     open the interface
  --probe            print a diagnostic (CPU, memory, GPU, temperatures...) and exit
  --version, -V      show the version
  --help, -h         show this help

Config: {path}
"
    )
}

fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None => run_tui(),
        Some("--version") | Some("-V") => {
            println!("guaitop {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") => {
            print!("{}", help_text());
            Ok(())
        }
        Some("--probe") => {
            run_probe();
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown argument: {other}\n\n{}", help_text());
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run_probe() {
    let ctl = Control::new(Duration::from_millis(500), None);
    ctl.set_procs_visible(true);
    let mut source = SystemSource::new(gpu::detect());
    let _ = source.sample(&ctl);
    std::thread::sleep(Duration::from_millis(500));
    print!("{}", probe::report(&source.sample(&ctl)));
}

fn run_tui() -> io::Result<()> {
    let (cfg, cfg_warning) = match config::default_path() {
        Some(p) => config::load(&p),
        None => (Config::default(), None),
    };
    let (keymap, key_warning) = match KeyMap::from_config(&cfg) {
        Ok(k) => (k, None),
        Err(e) => (
            KeyMap::from_config(&Config::default()).expect("default keys are valid"),
            Some(format!("invalid keys ({e}); using defaults")),
        ),
    };
    let truecolor = cfg
        .theme
        .truecolor
        .unwrap_or_else(|| detect_truecolor(std::env::var("COLORTERM").ok().as_deref()));
    let mut theme = Theme::new(truecolor, cfg.temps.warm, cfg.temps.hot);
    let ctl = Control::new(Duration::from_millis(cfg.refresh_ms), cfg.net_iface.clone());
    let rx = collect::spawn(ctl.clone(), || SystemSource::new(gpu::detect()));
    let mut app = App::new(&cfg);
    app.status = cfg_warning.or(key_warning);
    let mut disp = Dispatcher::new(keymap);

    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            ctl.stop();
            return Err(e);
        }
    };
    theme.graphics = Graphics::detect();
    update_cell_metrics(&mut theme);
    let events = merge_events(rx);
    let result = event_loop(
        &mut terminal,
        &mut app,
        &mut disp,
        &mut theme,
        &ctl,
        &events,
    );
    ratatui::restore();
    ctl.stop();
    result
}

/// Everything that can advance the interface, on a single channel.
enum Msg {
    Snapshot(Box<Snapshot>),
    Input(io::Result<Event>),
    CollectorGone,
}

/// Threads that block on the keyboard and on samples and forward them to a single channel: the main
/// loop sleeps until there is something to do (no periodic wakeups).
fn merge_events(snapshots: Receiver<Snapshot>) -> Receiver<Msg> {
    let (tx, rx) = mpsc::channel();
    let tx_input = tx.clone();
    let _ = thread::Builder::new()
        .name("snapshots".into())
        .spawn(move || {
            for s in snapshots {
                if tx.send(Msg::Snapshot(Box::new(s))).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::CollectorGone);
        });
    let _ = thread::Builder::new().name("input".into()).spawn(move || {
        loop {
            let e = event::read();
            let failed = e.is_err();
            if tx_input.send(Msg::Input(e)).is_err() || failed {
                return;
            }
        }
    });
    rx
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    disp: &mut Dispatcher,
    theme: &mut Theme,
    ctl: &Control,
    rx: &Receiver<Msg>,
) -> io::Result<()> {
    terminal.draw(|f| ui::draw(f, app, &*theme))?;
    loop {
        let Ok(first) = rx.recv() else {
            return Err(io::Error::other("the collector thread stopped"));
        };
        // Everything that has arrived is processed and drawn in a single pass.
        for msg in std::iter::once(first).chain(rx.try_iter()) {
            if handle(msg, app, disp, theme, ctl)? {
                return Ok(());
            }
        }
        terminal.draw(|f| ui::draw(f, app, &*theme))?;
    }
}

/// Applies a message to the state. Returns `true` if it is time to quit.
fn handle(
    msg: Msg,
    app: &mut App,
    disp: &mut Dispatcher,
    theme: &mut Theme,
    ctl: &Control,
) -> io::Result<bool> {
    let event = match msg {
        Msg::Snapshot(s) => {
            app.on_snapshot(*s);
            return Ok(false);
        }
        Msg::CollectorGone => return Err(io::Error::other("the collector thread stopped")),
        Msg::Input(e) => e?,
    };
    match event {
        Event::Key(k) if k.kind == KeyEventKind::Press => {
            let before = (app.tab, app.zoomed);
            let effects = app.on_key(&k, disp);
            // With another tab the terminal may have discarded the images.
            if (app.tab, app.zoomed) != before
                && let Some(g) = theme.graphics
            {
                g.invalidate();
            }
            for effect in effects {
                match effect {
                    Effect::Quit => return Ok(true),
                    Effect::Kill(pid) => {
                        app.status = Some(match collect::kill_process(pid) {
                            Ok(()) => format!("SIGTERM sent to {pid}"),
                            Err(e) => format!("could not kill {pid}: {e}"),
                        });
                    }
                    Effect::SetRefresh(d) => ctl.set_interval(d),
                    Effect::SetNetIface(i) => ctl.set_net_iface(i),
                    Effect::SetProcessesVisible(v) => ctl.set_procs_visible(v),
                }
            }
        }
        Event::Resize(..) => {
            // A font size change also arrives as Resize.
            update_cell_metrics(theme);
            if let Some(g) = theme.graphics {
                g.invalidate();
            }
        }
        _ => {}
    }
    Ok(false)
}

/// Size and aspect ratio of the current terminal cell. Prefers the real window size
/// (it changes with font zoom); if the terminal does not give it, the one reported by the
/// graphics protocol; and if neither, ratio 2.0 and no pixels (donuts are drawn with text).
fn update_cell_metrics(theme: &mut Theme) {
    let ws = ratatui::crossterm::terminal::window_size()
        .ok()
        .filter(|w| w.columns > 0 && w.rows > 0 && w.width > 0 && w.height > 0);
    if let Some(w) = ws {
        theme.cell_aspect = cell_aspect(w.columns, w.rows, w.width, w.height);
        theme.cell_px = Some((w.width / w.columns, w.height / w.rows));
    } else if let Some((fw, fh)) = theme.graphics.map(|g| g.font_size())
        && fw > 0
        && fh > 0
    {
        theme.cell_aspect = cell_aspect(1, 1, fw, fh);
        theme.cell_px = Some((fw, fh));
    } else {
        theme.cell_aspect = 2.0;
        theme.cell_px = None;
    }
}
