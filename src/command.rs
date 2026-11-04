//! `:` mode commands (k9s style): parsing, durations and autocompletion.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Mem,
    Gpu,
    Pid,
    Name,
}

impl SortKey {
    pub const ALL: [(&'static str, SortKey); 5] = [
        ("cpu", SortKey::Cpu),
        ("mem", SortKey::Mem),
        ("gpu", SortKey::Gpu),
        ("pid", SortKey::Pid),
        ("name", SortKey::Name),
    ];

    pub fn label(self) -> &'static str {
        SortKey::ALL
            .iter()
            .find(|(_, k)| *k == self)
            .map(|(n, _)| *n)
            .unwrap_or("cpu")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Dash,
    Proc,
    Sort(SortKey),
    Filter(Option<String>),
    Kill(u32),
    Refresh(Duration),
    /// 0-based index (the user types 1-based).
    Gpu(usize),
    /// `None` = automatic (physical interfaces).
    Net(Option<String>),
    Quit,
}

pub const COMMANDS: [&str; 9] = [
    "dash", "proc", "sort", "filter", "kill", "refresh", "gpu", "net", "q",
];

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim().trim_start_matches(':').trim();
    let (name, arg) = match input.split_once(char::is_whitespace) {
        Some((n, a)) => (n, Some(a.trim()).filter(|a| !a.is_empty())),
        None => (input, None),
    };
    match name {
        "" => Err("ordre buida".into()),
        "dash" => Ok(Command::Dash),
        "proc" => Ok(Command::Proc),
        "q" | "quit" => Ok(Command::Quit),
        "sort" => {
            let a = arg.ok_or("ús: :sort cpu|mem|gpu|pid|name")?;
            SortKey::ALL
                .iter()
                .find(|(n, _)| *n == a)
                .map(|(_, k)| Command::Sort(*k))
                .ok_or_else(|| format!("criteri desconegut: {a} (cpu|mem|gpu|pid|name)"))
        }
        "filter" => Ok(Command::Filter(arg.map(str::to_string))),
        "kill" => {
            let a = arg.ok_or("ús: :kill <pid>")?;
            match a.parse::<u32>() {
                Ok(pid) if (1..=i32::MAX as u32).contains(&pid) => Ok(Command::Kill(pid)),
                _ => Err(format!("pid invàlid: {a} (1..={})", i32::MAX)),
            }
        }
        "refresh" => {
            let a = arg.ok_or("ús: :refresh <durada> (p. ex. 500ms, 2s)")?;
            parse_duration(a).map(Command::Refresh)
        }
        "gpu" => {
            let a = arg.ok_or("ús: :gpu <n>")?;
            match a.parse::<usize>() {
                Ok(n) if n >= 1 => Ok(Command::Gpu(n - 1)),
                _ => Err(format!("número de GPU invàlid: {a}")),
            }
        }
        "net" => Ok(Command::Net(
            arg.filter(|a| *a != "auto").map(str::to_string),
        )),
        other => Err(format!("ordre desconeguda: {other}")),
    }
}

/// "500ms", "2s", "1.5s". Allowed range: 100ms..=60s.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let invalid = || format!("durada invàlida: {s} (usa ms o s)");
    let ms: u64 = if let Some(n) = s.strip_suffix("ms") {
        n.trim().parse().map_err(|_| invalid())?
    } else if let Some(n) = s.strip_suffix('s') {
        let secs: f64 = n.trim().parse().map_err(|_| invalid())?;
        if !secs.is_finite() || secs < 0.0 {
            return Err(invalid());
        }
        (secs * 1000.0).round() as u64
    } else {
        return Err(invalid());
    };
    if !(100..=60_000).contains(&ms) {
        return Err("la durada ha d'estar entre 100ms i 60s".into());
    }
    Ok(Duration::from_millis(ms))
}

/// Candidates for the last token of the input.
pub fn complete(input: &str) -> Vec<String> {
    let input = input.trim_start_matches(':');
    match input.split_once(' ') {
        None => COMMANDS
            .iter()
            .filter(|c| c.starts_with(input))
            .map(|c| c.to_string())
            .collect(),
        Some(("sort", rest)) => SortKey::ALL
            .iter()
            .map(|(n, _)| *n)
            .filter(|n| n.starts_with(rest.trim_start()))
            .map(str::to_string)
            .collect(),
        Some(("net", rest)) => ["auto"]
            .iter()
            .filter(|n| n.starts_with(rest.trim_start()))
            .map(|n| n.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// If there is exactly one candidate, returns the completed input.
pub fn apply_completion(input: &str) -> Option<String> {
    let candidates = complete(input);
    if candidates.len() != 1 {
        return None;
    }
    let input = input.trim_start_matches(':');
    Some(match input.split_once(' ') {
        None => format!("{} ", candidates[0]),
        Some((cmd, _)) => format!("{cmd} {}", candidates[0]),
    })
}
