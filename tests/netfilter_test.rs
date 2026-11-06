use guaitop::collect::netfilter::*;

#[test]
fn physical_interfaces() {
    for n in ["enp0s31f6", "enp0s13f0u1u3", "en0", "eth0", "wlan0"] {
        assert!(is_physical(n), "{n}");
    }
    for n in [
        "lo",
        "lo0",
        "docker0",
        "veth7c5de12",
        "br-aa7110bb4894",
        "tailscale0",
        "utun3",
        "awdl0",
        "llw0",
        "bridge0",
        "virbr0",
        "wg0",
    ] {
        assert!(!is_physical(n), "{n}");
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn selects_physical_by_default() {
    let all = names(&["lo", "enp0s31f6", "docker0", "enp0s13f0u1u3"]);
    let s = select(&all, None);
    assert_eq!(s.names, names(&["enp0s13f0u1u3", "enp0s31f6"]));
    assert_eq!(s.label, "2 physical");
    let s = select(&names(&["lo", "enp0s31f6"]), None);
    assert_eq!(s.label, "enp0s31f6");
    let s = select(&names(&["lo", "docker0"]), None);
    assert!(s.names.is_empty());
    assert_eq!(s.label, "no physical interface");
}

#[test]
fn pinned_interface_wins() {
    let all = names(&["lo", "enp0s31f6", "docker0"]);
    let s = select(&all, Some("docker0"));
    assert_eq!(s.names, names(&["docker0"]));
    assert_eq!(s.label, "docker0");
    let s = select(&all, Some("eth9"));
    assert!(s.names.is_empty());
    assert_eq!(s.label, "eth9 (not found)");
}
