use std::io;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use observability::app::{App, Effect};
use observability::collect::system::SystemSource;
use observability::collect::{self, Control, Source};
use observability::config::{self, Config};
use observability::keys::{Dispatcher, KeyMap};
use observability::model::Snapshot;
use observability::ui::{self, theme::Theme, theme::detect_truecolor};
use observability::{gpu, probe};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn help_text() -> String {
    let path = config::default_path().map_or_else(
        || "(no disponible)".to_string(),
        |p| p.display().to_string(),
    );
    format!(
        "observability — monitor de recursos en terminal

Ús: observability [--probe | --version | --help]

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
            println!("observability {}", env!("CARGO_PKG_VERSION"));
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
    let theme = Theme::new(truecolor, cfg.temps.warm, cfg.temps.hot);
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
    let result = event_loop(&mut terminal, &mut app, &mut disp, &theme, &ctl, &rx);
    ratatui::restore();
    ctl.stop();
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    disp: &mut Dispatcher,
    theme: &Theme,
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
            terminal.draw(|f| ui::draw(f, app, theme))?;
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
            Event::Resize(..) => dirty = true,
            _ => {}
        }
    }
}
