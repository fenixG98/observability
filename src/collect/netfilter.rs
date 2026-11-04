//! Which network interfaces are measured.

const VIRTUAL_PREFIXES: &[&str] = &[
    "lo",
    "docker",
    "veth",
    "br-",
    "virbr",
    "vnet",
    "tun",
    "tap",
    "wg",
    "tailscale",
    "zt",
    "utun",
    "awdl",
    "llw",
    "bridge",
    "gif",
    "stf",
    "anpi",
    "ap",
    "vmnet",
    "vboxnet",
    "cni",
    "flannel",
    "cali",
    "kube",
    "podman",
    "lxc",
    "lxd",
    "dummy",
    "ipsec",
    "ppp",
];

/// Name-based heuristic: known virtual interfaces are excluded.
pub fn is_physical(name: &str) -> bool {
    !VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetSelection {
    /// Interfaces to sum, sorted.
    pub names: Vec<String>,
    /// Text for the panel header.
    pub label: String,
}

pub fn select(names: &[String], pinned: Option<&str>) -> NetSelection {
    if let Some(p) = pinned {
        return if names.iter().any(|n| n == p) {
            NetSelection {
                names: vec![p.to_string()],
                label: p.to_string(),
            }
        } else {
            NetSelection {
                names: vec![],
                label: format!("{p} (no trobada)"),
            }
        };
    }
    let mut chosen: Vec<String> = names.iter().filter(|n| is_physical(n)).cloned().collect();
    chosen.sort();
    let label = match chosen.len() {
        0 => "cap interfície física".to_string(),
        1 => chosen[0].clone(),
        n => format!("{n} físiques"),
    };
    NetSelection {
        names: chosen,
        label,
    }
}
