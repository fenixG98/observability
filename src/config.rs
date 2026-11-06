//! TOML configuration with default values and validation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub refresh_ms: u64,
    pub history_secs: u64,
    pub net_iface: Option<String>,
    pub prefix: PrefixConfig,
    pub keys: KeysConfig,
    pub theme: ThemeConfig,
    pub temps: TempThresholds,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            refresh_ms: 1000,
            history_secs: 60,
            net_iface: None,
            prefix: PrefixConfig::default(),
            keys: KeysConfig::default(),
            theme: ThemeConfig::default(),
            temps: TempThresholds::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PrefixConfig {
    pub enabled: bool,
    pub key: String,
}

impl Default for PrefixConfig {
    fn default() -> Self {
        PrefixConfig {
            enabled: false,
            key: "ctrl+b".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeysConfig {
    pub tab_dashboard: String,
    pub tab_processes: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub left: String,
    pub down: String,
    pub up: String,
    pub right: String,
    pub zoom: String,
    pub gpu_next: String,
    pub command: String,
    pub filter: String,
    pub help: String,
    pub quit: String,
    pub kill: String,
}

impl Default for KeysConfig {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        KeysConfig {
            tab_dashboard: s("1"),
            tab_processes: s("2"),
            next_tab: s("n"),
            prev_tab: s("p"),
            left: s("h"),
            down: s("j"),
            up: s("k"),
            right: s("l"),
            zoom: s("z"),
            gpu_next: s("g"),
            command: s(":"),
            filter: s("/"),
            help: s("?"),
            quit: s("q"),
            kill: s("x"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeConfig {
    /// `None` = detect with `$COLORTERM`.
    pub truecolor: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TempThresholds {
    pub warm: f32,
    pub hot: f32,
}

impl Default for TempThresholds {
    fn default() -> Self {
        TempThresholds {
            warm: 60.0,
            hot: 80.0,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(100..=60_000).contains(&self.refresh_ms) {
            return Err(format!(
                "refresh_ms must be between 100 and 60000 (got {})",
                self.refresh_ms
            ));
        }
        if !(10..=3600).contains(&self.history_secs) {
            return Err(format!(
                "history_secs must be between 10 and 3600 (got {})",
                self.history_secs
            ));
        }
        if self.temps.warm.partial_cmp(&self.temps.hot) != Some(std::cmp::Ordering::Less) {
            return Err("temps.warm must be lower than temps.hot".into());
        }
        Ok(())
    }

    /// Number of samples that fit in the visible history.
    pub fn history_len(&self) -> usize {
        (self.history_secs.saturating_mul(1000) / self.refresh_ms.max(1)).max(2) as usize
    }
}

pub fn parse(text: &str) -> Result<Config, String> {
    let c: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    c.validate()?;
    Ok(c)
}

/// Loads the configuration. If the file does not exist, defaults without a warning;
/// if it is invalid, defaults with a warning to show to the user.
pub fn load(path: &Path) -> (Config, Option<String>) {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), None),
        Err(e) => (
            Config::default(),
            Some(format!("could not read {}: {e}", path.display())),
        ),
        Ok(text) => match parse(&text) {
            Ok(c) => (c, None),
            Err(e) => (
                Config::default(),
                Some(format!(
                    "invalid config in {} ({e}); using defaults",
                    path.display()
                )),
            ),
        },
    }
}

/// `~/.config/guaitop/config.toml` (Linux) or
/// `~/Library/Application Support/guaitop/config.toml` (macOS).
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "guaitop").map(|d| d.config_dir().join("config.toml"))
}
