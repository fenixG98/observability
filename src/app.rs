//! Application state: tabs, focus, modes (commands, filter, confirmation) and histories.
//! Has no side effects: returns `Effect`s that `main` executes.

use std::collections::HashMap;
use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::command::{self, Command, SortKey};
use crate::config::Config;
use crate::history::History;
use crate::keys::{Action, Dir, Dispatcher, KeyContext};
use crate::model::{GpuReading, ProcInfo, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Processes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Command {
        input: String,
        error: Option<String>,
    },
    Filter {
        input: String,
    },
    ConfirmKill {
        pid: u32,
        name: String,
    },
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    CpuDonut,
    MemDonut,
    GpuDonut,
    DiskDonut,
    History,
    Temps,
    Net,
    DiskIo,
    ProcTable,
    ProcPie,
    ProcDetail,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Quit,
    Kill(u32),
    SetRefresh(Duration),
    SetNetIface(Option<String>),
    SetProcessesVisible(bool),
}

#[derive(Debug, Clone)]
pub struct Histories {
    /// Total CPU usage, 0..=1.
    pub cpu: History,
    /// Per-GPU usage (same order as `Snapshot::gpus`), 0..=1.
    pub gpu: Vec<History>,
    /// Bytes/s.
    pub rx: History,
    pub tx: History,
    pub disk_r: History,
    pub disk_w: History,
    /// CPU % of the selected process.
    pub proc_cpu: HashMap<u32, History>,
}

impl Histories {
    fn new(len: usize) -> Histories {
        Histories {
            cpu: History::new(len),
            gpu: Vec::new(),
            rx: History::new(len),
            tx: History::new(len),
            disk_r: History::new(len),
            disk_w: History::new(len),
            proc_cpu: HashMap::new(),
        }
    }

    fn set_len(&mut self, len: usize) {
        for h in [
            &mut self.cpu,
            &mut self.rx,
            &mut self.tx,
            &mut self.disk_r,
            &mut self.disk_w,
        ] {
            h.set_capacity(len);
        }
        for h in self.gpu.iter_mut().chain(self.proc_cpu.values_mut()) {
            h.set_capacity(len);
        }
    }
}

/// Neighbor of a panel in one direction (layout B grid and processes tab).
pub fn neighbor(p: Panel, d: Dir) -> Option<Panel> {
    use Dir::*;
    use Panel::*;
    match (p, d) {
        (CpuDonut, Right) => Some(MemDonut),
        (CpuDonut, Down) => Some(History),
        (MemDonut, Left) => Some(CpuDonut),
        (MemDonut, Right) => Some(GpuDonut),
        (MemDonut, Down) => Some(History),
        (GpuDonut, Left) => Some(MemDonut),
        (GpuDonut, Right) => Some(DiskDonut),
        (GpuDonut, Down) => Some(History),
        (DiskDonut, Left) => Some(GpuDonut),
        (DiskDonut, Down) => Some(Temps),
        (History, Up) => Some(CpuDonut),
        (History, Right) => Some(Temps),
        (History, Down) => Some(Net),
        (Temps, Up) => Some(DiskDonut),
        (Temps, Left) => Some(History),
        (Temps, Down) => Some(DiskIo),
        (Net, Up) => Some(History),
        (Net, Right) => Some(DiskIo),
        (DiskIo, Up) => Some(Temps),
        (DiskIo, Left) => Some(Net),
        (ProcTable, Right) => Some(ProcPie),
        (ProcPie, Left) => Some(ProcTable),
        (ProcPie, Down) => Some(ProcDetail),
        (ProcDetail, Left) => Some(ProcTable),
        (ProcDetail, Up) => Some(ProcPie),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct App {
    pub tab: Tab,
    pub mode: Mode,
    pub focus: Panel,
    pub zoomed: bool,
    pub sort: SortKey,
    pub filter: Option<String>,
    /// Index into `visible_procs()`.
    pub selected: usize,
    pub gpu_index: usize,
    pub refresh: Duration,
    pub history_secs: u64,
    pub snapshot: Snapshot,
    pub hist: Histories,
    /// Message for the status line (warnings, command results).
    pub status: Option<String>,
    pub prefix_armed: bool,
}

impl App {
    pub fn new(cfg: &Config) -> App {
        App {
            tab: Tab::Dashboard,
            mode: Mode::Normal,
            focus: Panel::CpuDonut,
            zoomed: false,
            sort: SortKey::Cpu,
            filter: None,
            selected: 0,
            gpu_index: 0,
            refresh: Duration::from_millis(cfg.refresh_ms),
            history_secs: cfg.history_secs,
            snapshot: Snapshot::default(),
            hist: Histories::new(cfg.history_len()),
            status: None,
            prefix_armed: false,
        }
    }

    pub fn history_len(&self) -> usize {
        let ms = (self.refresh.as_millis() as u64).max(1);
        (self.history_secs.saturating_mul(1000) / ms).max(2) as usize
    }

    pub fn gpu_count(&self) -> usize {
        self.snapshot.gpus.ok().map_or(0, |v| v.len())
    }

    pub fn current_gpu(&self) -> Option<&GpuReading> {
        self.snapshot.gpus.ok()?.get(self.gpu_index)
    }

    /// Filtered and sorted processes (empty if the processes tab has received no data).
    pub fn visible_procs(&self) -> Vec<&ProcInfo> {
        let Some(procs) = &self.snapshot.procs else {
            return Vec::new();
        };
        let needle = self.filter.as_deref().map(str::to_lowercase);
        let mut v: Vec<&ProcInfo> = procs
            .iter()
            .filter(|p| match &needle {
                None => true,
                Some(n) => {
                    p.name.to_lowercase().contains(n.as_str())
                        || p.pid.to_string().starts_with(n.as_str())
                }
            })
            .collect();
        match self.sort {
            SortKey::Cpu => v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(a.pid.cmp(&b.pid))),
            SortKey::Mem => v.sort_by(|a, b| b.mem.cmp(&a.mem).then(a.pid.cmp(&b.pid))),
            SortKey::Gpu => v.sort_by(|a, b| {
                b.gpu
                    .unwrap_or(-1.0)
                    .total_cmp(&a.gpu.unwrap_or(-1.0))
                    .then(a.pid.cmp(&b.pid))
            }),
            SortKey::Pid => v.sort_by_key(|p| p.pid),
            SortKey::Name => v.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then(a.pid.cmp(&b.pid))
            }),
        }
        v
    }

    pub fn selected_proc(&self) -> Option<&ProcInfo> {
        self.visible_procs().get(self.selected).copied()
    }

    /// CPU fraction of the top `n` processes, plus "other" (sums to 1).
    pub fn cpu_share(&self, n: usize) -> Vec<(String, f64)> {
        let Some(procs) = &self.snapshot.procs else {
            return Vec::new();
        };
        let total: f64 = procs.iter().map(|p| p.cpu.max(0.0) as f64).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        let mut sorted: Vec<&ProcInfo> = procs.iter().collect();
        sorted.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        let mut out: Vec<(String, f64)> = sorted
            .iter()
            .take(n)
            .map(|p| (p.name.clone(), p.cpu.max(0.0) as f64 / total))
            .collect();
        let rest = 1.0 - out.iter().map(|s| s.1).sum::<f64>();
        if rest > 1e-9 {
            out.push(("other".into(), rest));
        }
        out
    }

    pub fn on_snapshot(&mut self, s: Snapshot) {
        let len = self.history_len();
        self.hist.cpu.push(s.cpu.total as f64 / 100.0);
        let gpus: &[GpuReading] = s.gpus.ok().map_or(&[], |v| v.as_slice());
        self.hist.gpu.resize_with(gpus.len(), || History::new(len));
        for (h, g) in self.hist.gpu.iter_mut().zip(gpus) {
            h.push(g.util.ok().map_or(0.0, |u| *u as f64 / 100.0));
        }
        if self.gpu_index >= gpus.len() {
            self.gpu_index = 0;
        }
        self.hist.rx.push(s.net.rx_bps as f64);
        self.hist.tx.push(s.net.tx_bps as f64);
        self.hist.disk_r.push(s.disk.read_bps as f64);
        self.hist.disk_w.push(s.disk.write_bps as f64);
        self.snapshot = s;

        let n = self.visible_procs().len();
        self.selected = if n == 0 { 0 } else { self.selected.min(n - 1) };
        match self.selected_proc().map(|p| (p.pid, p.cpu)) {
            Some((pid, cpu)) => {
                self.hist.proc_cpu.retain(|k, _| *k == pid);
                self.hist
                    .proc_cpu
                    .entry(pid)
                    .or_insert_with(|| History::new(len))
                    .push(cpu as f64);
            }
            None => self.hist.proc_cpu.clear(),
        }
    }

    pub fn on_key(&mut self, ev: &KeyEvent, disp: &mut Dispatcher) -> Vec<Effect> {
        match self.mode.clone() {
            Mode::Normal => {
                let ctx = KeyContext {
                    processes_tab: self.tab == Tab::Processes,
                };
                let action = disp.handle(ev, ctx);
                self.prefix_armed = disp.prefix_armed();
                match action {
                    Some(a) => {
                        self.status = None;
                        self.apply_action(a)
                    }
                    None => Vec::new(),
                }
            }
            Mode::Command { mut input, .. } => match ev.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Vec::new()
                }
                KeyCode::Enter => match command::parse(&input) {
                    Ok(c) => {
                        self.mode = Mode::Normal;
                        self.apply_command(c)
                    }
                    Err(e) => {
                        self.mode = Mode::Command {
                            input,
                            error: Some(e),
                        };
                        Vec::new()
                    }
                },
                KeyCode::Tab => {
                    if let Some(done) = command::apply_completion(&input) {
                        input = done;
                    }
                    self.mode = Mode::Command { input, error: None };
                    Vec::new()
                }
                KeyCode::Backspace => {
                    self.mode = if input.pop().is_none() {
                        Mode::Normal
                    } else {
                        Mode::Command { input, error: None }
                    };
                    Vec::new()
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    self.mode = Mode::Command { input, error: None };
                    Vec::new()
                }
                _ => Vec::new(),
            },
            Mode::Filter { mut input } => {
                match ev.code {
                    KeyCode::Esc => {
                        self.filter = None;
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Enter => self.mode = Mode::Normal,
                    KeyCode::Backspace => {
                        input.pop();
                        self.set_filter_live(input);
                    }
                    KeyCode::Char(c) => {
                        input.push(c);
                        self.set_filter_live(input);
                    }
                    _ => {}
                }
                Vec::new()
            }
            Mode::ConfirmKill { pid, name } => {
                self.mode = Mode::Normal;
                match ev.code {
                    KeyCode::Char('s') | KeyCode::Char('y') | KeyCode::Enter => {
                        self.status = Some(format!("sending SIGTERM to {name} ({pid})"));
                        vec![Effect::Kill(pid)]
                    }
                    _ => {
                        self.status = Some("cancelled".into());
                        Vec::new()
                    }
                }
            }
            Mode::Help => {
                self.mode = Mode::Normal;
                Vec::new()
            }
        }
    }

    fn set_filter_live(&mut self, input: String) {
        self.filter = if input.is_empty() {
            None
        } else {
            Some(input.clone())
        };
        self.selected = 0;
        self.mode = Mode::Filter { input };
    }

    fn set_tab(&mut self, tab: Tab) -> Vec<Effect> {
        if self.tab == tab {
            return Vec::new();
        }
        self.tab = tab;
        self.zoomed = false;
        self.focus = match tab {
            Tab::Dashboard => Panel::CpuDonut,
            Tab::Processes => Panel::ProcTable,
        };
        vec![Effect::SetProcessesVisible(tab == Tab::Processes)]
    }

    pub fn apply_action(&mut self, a: Action) -> Vec<Effect> {
        match a {
            Action::Tab(0) => self.set_tab(Tab::Dashboard),
            Action::Tab(1) => self.set_tab(Tab::Processes),
            Action::Tab(_) => Vec::new(),
            Action::NextTab | Action::PrevTab => {
                let other = if self.tab == Tab::Dashboard {
                    Tab::Processes
                } else {
                    Tab::Dashboard
                };
                self.set_tab(other)
            }
            Action::Focus(d) => {
                if !self.zoomed
                    && let Some(p) = neighbor(self.focus, d)
                {
                    self.focus = p;
                }
                Vec::new()
            }
            Action::Zoom => {
                self.zoomed = !self.zoomed;
                Vec::new()
            }
            Action::GpuNext => {
                let n = self.gpu_count();
                if n > 0 {
                    self.gpu_index = (self.gpu_index + 1) % n;
                }
                Vec::new()
            }
            Action::OpenCommand => {
                self.mode = Mode::Command {
                    input: String::new(),
                    error: None,
                };
                Vec::new()
            }
            Action::OpenFilter => {
                let fx = self.set_tab(Tab::Processes);
                self.mode = Mode::Filter {
                    input: self.filter.clone().unwrap_or_default(),
                };
                fx
            }
            Action::Help => {
                self.mode = Mode::Help;
                Vec::new()
            }
            Action::Quit => vec![Effect::Quit],
            Action::Kill => {
                if self.tab == Tab::Processes
                    && let Some((pid, name)) = self.selected_proc().map(|p| (p.pid, p.name.clone()))
                {
                    self.mode = Mode::ConfirmKill { pid, name };
                }
                Vec::new()
            }
            Action::RowUp => {
                self.selected = self.selected.saturating_sub(1);
                Vec::new()
            }
            Action::RowDown => {
                if self.selected + 1 < self.visible_procs().len() {
                    self.selected += 1;
                }
                Vec::new()
            }
        }
    }

    pub fn apply_command(&mut self, c: Command) -> Vec<Effect> {
        match c {
            Command::Dash => self.set_tab(Tab::Dashboard),
            Command::Proc => self.set_tab(Tab::Processes),
            Command::Sort(k) => {
                self.sort = k;
                self.selected = 0;
                self.status = Some(format!("sort: {}", k.label()));
                Vec::new()
            }
            Command::Filter(f) => {
                self.filter = f;
                self.selected = 0;
                Vec::new()
            }
            Command::Kill(pid) => {
                let name = self
                    .snapshot
                    .procs
                    .as_ref()
                    .and_then(|ps| ps.iter().find(|p| p.pid == pid))
                    .map_or_else(|| "unknown process".to_string(), |p| p.name.clone());
                self.mode = Mode::ConfirmKill { pid, name };
                Vec::new()
            }
            Command::Refresh(d) => {
                self.refresh = d;
                let len = self.history_len();
                self.hist.set_len(len);
                self.status = Some(format!("refresh: {} ms", d.as_millis()));
                vec![Effect::SetRefresh(d)]
            }
            Command::Gpu(i) => {
                let n = self.gpu_count();
                if i < n {
                    self.gpu_index = i;
                } else {
                    self.status = Some(format!("no GPU {} (there are {n})", i + 1));
                }
                Vec::new()
            }
            Command::Net(iface) => {
                self.status = Some(match &iface {
                    Some(i) => format!("network: {i}"),
                    None => "network: physical interfaces".into(),
                });
                vec![Effect::SetNetIface(iface)]
            }
            Command::Quit => vec![Effect::Quit],
        }
    }
}
