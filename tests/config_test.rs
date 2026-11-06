use guaitop::config::{self, Config};
use std::path::Path;

#[test]
fn defaults_match_spec() {
    let c = Config::default();
    assert_eq!(c.refresh_ms, 1000);
    assert_eq!(c.history_secs, 60);
    assert_eq!(c.net_iface, None);
    assert!(!c.prefix.enabled);
    assert_eq!(c.prefix.key, "ctrl+b");
    assert_eq!(c.keys.quit, "q");
    assert_eq!(c.keys.gpu_next, "g");
    assert_eq!(c.theme.truecolor, None);
    assert_eq!(c.temps.warm, 60.0);
    assert_eq!(c.temps.hot, 80.0);
    assert_eq!(c.history_len(), 60);
}

#[test]
fn partial_file_merges_with_defaults() {
    let c = config::parse("refresh_ms = 500\n[prefix]\nenabled = true\n").unwrap();
    assert_eq!(c.refresh_ms, 500);
    assert!(c.prefix.enabled);
    assert_eq!(c.prefix.key, "ctrl+b");
    assert_eq!(c.history_len(), 120);
}

#[test]
fn unknown_keys_are_rejected() {
    assert!(config::parse("refrsh_ms = 5").is_err());
    assert!(config::parse("[keys]\nquitt = \"q\"").is_err());
}

#[test]
fn out_of_range_values_are_rejected() {
    assert!(
        config::parse("refresh_ms = 10")
            .unwrap_err()
            .contains("refresh_ms")
    );
    assert!(
        config::parse("history_secs = 5")
            .unwrap_err()
            .contains("history_secs")
    );
    assert!(
        config::parse("[temps]\nwarm = 90.0\nhot = 80.0")
            .unwrap_err()
            .contains("warm")
    );
}

#[test]
fn missing_file_gives_defaults_without_warning() {
    let (c, w) = config::load(Path::new("/nonexistent/guaitop/config.toml"));
    assert_eq!(c, Config::default());
    assert!(w.is_none());
}

#[test]
fn invalid_file_gives_defaults_with_warning() {
    let dir = std::env::temp_dir().join(format!("obs-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "refresh_ms = \"fast\"").unwrap();
    let (c, w) = config::load(&path);
    assert_eq!(c, Config::default());
    assert!(w.unwrap().contains("invalid config"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn example_config_equals_defaults() {
    let text = include_str!("../config/config.example.toml");
    assert_eq!(config::parse(text).unwrap(), Config::default());
}

#[test]
fn default_path_ends_with_config_toml() {
    let p = config::default_path().expect("a config directory exists");
    assert!(p.ends_with("guaitop/config.toml"));
}

#[test]
fn nan_temp_thresholds_are_rejected() {
    assert!(config::parse("[temps]\nwarm = nan\n").is_err());
    assert!(config::parse("[temps]\nhot = nan\n").is_err());
}

#[test]
fn history_len_survives_zero_refresh() {
    let c = Config {
        refresh_ms: 0,
        ..Config::default()
    };
    assert!(c.history_len() >= 2);
}
