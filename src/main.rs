use std::io;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use observability::app::{App, Effect};
use observability::collect::system::SystemSource;
use observability::collect::{self, Control, Source};
use observability::config::{self, Config};
use observability::keys::{Dispatcher, KeyMap};
use observability::model::Snapshot;
use observability::ui::{
    self,
    pixels::Graphics,
    theme::{Theme, cell_aspect, detect_truecolor},
};
use observability::{gpu, probe};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn help_text() -> String {
    let path = config::default_path().map_or_else(
        || "(no disponible)".to_string(),
        |p| p.display().to_string(),
    );
    format!(
        "obstui — monitor de recursos en terminal

Ús: obstui [--probe | --version | --help]

  (sense arguments)  obre la interfície
  --probe            imprimeix un diagnòstic (CPU, memòria, GPU, temperatures...) i surt
  --version, -V      mostra la versió
  --help, -h         mostra aquesta ajuda

Configuració: {path}
"
    )
}

fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None => run_tui(),
        Some("--version") | Some("-V") => {
            println!("obstui {}", env!("CARGO_PKG_VERSION"));
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
            eprintln!("argument desconegut: {other}\n\n{}", help_text());
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
            KeyMap::from_config(&Config::default()).expect("the default keys are valid"),
            Some(format!("tecles invàlides ({e}); s'usen les per defecte")),
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
    let result = event_loop(&mut terminal, &mut app, &mut disp, &mut theme, &ctl, &rx);
    ratatui::restore();
    ctl.stop();
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    disp: &mut Dispatcher,
    theme: &mut Theme,
    ctl: &Control,
    rx: &Receiver<Snapshot>,
) -> io::Result<()> {
    let mut dirty = true;
    loop {
        loop {
            match rx.try_recv() {
                Ok(s) => {
                    app.on_snapshot(s);
                    dirty = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::other("el fil recol·lector s'ha aturat"));
                }
            }
        }
        if dirty {
            terminal.draw(|f| ui::draw(f, app, &*theme))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                dirty = true;
                for effect in app.on_key(&k, disp) {
                    match effect {
                        Effect::Quit => return Ok(()),
                        Effect::Kill(pid) => {
                            app.status = Some(match collect::kill_process(pid) {
                                Ok(()) => format!("SIGTERM enviat a {pid}"),
                                Err(e) => format!("no s'ha pogut matar {pid}: {e}"),
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
                dirty = true;
            }
            _ => {}
        }
    }
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
