//! System metrics collection in a dedicated thread.

pub mod meminfo;
pub mod netfilter;
pub mod procfs;
pub mod system;
pub mod temps;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::model::Snapshot;

/// Parameters the UI changes live and the collector thread reads.
#[derive(Debug, Default)]
pub struct Control {
    interval_ms: AtomicU64,
    procs_visible: AtomicBool,
    stop: AtomicBool,
    net_iface: Mutex<Option<String>>,
    /// Wakes the collector thread when the interval changes or it is stopped (no polling).
    wake: Condvar,
    wake_lock: Mutex<()>,
}

impl Control {
    pub fn new(interval: Duration, net_iface: Option<String>) -> Arc<Control> {
        Arc::new(Control {
            interval_ms: AtomicU64::new(interval.as_millis() as u64),
            procs_visible: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            net_iface: Mutex::new(net_iface),
            wake: Condvar::new(),
            wake_lock: Mutex::new(()),
        })
    }

    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms.load(Ordering::Relaxed))
    }

    pub fn set_interval(&self, d: Duration) {
        self.interval_ms
            .store(d.as_millis() as u64, Ordering::Relaxed);
        self.notify();
    }

    pub fn procs_visible(&self) -> bool {
        self.procs_visible.load(Ordering::Relaxed)
    }

    pub fn set_procs_visible(&self, v: bool) {
        self.procs_visible.store(v, Ordering::Relaxed);
    }

    pub fn net_iface(&self) -> Option<String> {
        self.net_iface.lock().map(|g| g.clone()).unwrap_or(None)
    }

    pub fn set_net_iface(&self, iface: Option<String>) {
        if let Ok(mut g) = self.net_iface.lock() {
            *g = iface;
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.notify();
    }

    fn notify(&self) {
        let _guard = self.wake_lock.lock();
        self.wake.notify_all();
    }

    /// Waits until the interval has elapsed since `since` or until the thread is stopped.
    /// Re-reads the interval on every wakeup, so a change takes effect right away.
    fn sleep_until_next(&self, since: Instant) {
        let Ok(mut guard) = self.wake_lock.lock() else {
            return;
        };
        loop {
            let elapsed = since.elapsed();
            let interval = self.interval();
            if self.stopped() || elapsed >= interval {
                return;
            }
            match self.wake.wait_timeout(guard, interval - elapsed) {
                Ok((g, _)) => guard = g,
                Err(_) => return,
            }
        }
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// Sample source. Built inside the collector thread (does not need to be `Send`).
pub trait Source {
    fn sample(&mut self, ctl: &Control) -> Snapshot;
}

/// Spawns the collector thread. Publishes a sample immediately and then every `interval`.
/// The thread ends when `ctl.stop()` is called or when the receiver is dropped.
pub fn spawn<S, F>(ctl: Arc<Control>, make: F) -> mpsc::Receiver<Snapshot>
where
    S: Source,
    F: FnOnce() -> S + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    // If the thread cannot be created, `tx` is dropped with the closure and the receiver
    // ends up disconnected: the consumer must treat it as a fatal error.
    let _ = thread::Builder::new()
        .name("collector".into())
        .spawn(move || {
            let mut source = make();
            while !ctl.stopped() {
                if tx.send(source.sample(&ctl)).is_err() {
                    break;
                }
                ctl.sleep_until_next(Instant::now());
            }
        });
    rx
}

/// Sends SIGTERM to `pid`. Rejects pid 0 (the whole process group), pid 1 (init),
/// the current process and pids that do not fit in a `pid_t` (they would become negative,
/// which `kill` interprets as groups).
pub fn kill_process(pid: u32) -> Result<(), String> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(format!("invalid pid: {pid}"));
    }
    if pid == 1 {
        return Err("pid 1 (init) is protected".into());
    }
    if pid == std::process::id() {
        return Err("refusing to kill guaitop itself".into());
    }
    // SAFETY: system call with no pointers.
    let r = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}
