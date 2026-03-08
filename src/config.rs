use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.toml";

/// Sentinel value for `lock_in_*_secs` meaning "lock in as soon as hovered".
pub const INSTANT: u64 = u64::MAX;

/// Human-readable representation of a lock-in threshold.
pub fn format_lock_in(secs: u64) -> String {
    if secs == INSTANT {
        "Instant".to_string()
    } else {
        format!("≤ {secs}s remaining")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Override the lockfile path. If None, common Windows paths are scanned.
    pub lockfile_path: Option<String>,
    /// Champions to ban, in priority order (first = most preferred ban).
    #[serde(default)]
    pub bans: Vec<String>,
    /// Lock in the ban when the timer has this many seconds or fewer remaining.
    #[serde(default = "default_lock_in_ban_secs")]
    pub lock_in_ban_secs: u64,
    /// Lock in the champion pick when the timer has this many seconds or fewer remaining.
    #[serde(default = "default_lock_in_pick_secs")]
    pub lock_in_pick_secs: u64,
    pub preferences: LanePreferences,
}

fn default_lock_in_ban_secs() -> u64 {
    5
}

fn default_lock_in_pick_secs() -> u64 {
    10
}

/// Champion preferences per lane. Listed in priority order (first = most preferred).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LanePreferences {
    pub top: Vec<String>,
    pub jungle: Vec<String>,
    pub mid: Vec<String>,
    /// Bot / ADC lane
    pub bot: Vec<String>,
    pub support: Vec<String>,
    /// Fallback when position is FILL or unrecognised
    pub fill: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lockfile_path: None,
            bans: vec!["Zed".into(), "Yasuo".into(), "Yone".into()],
            lock_in_ban_secs: default_lock_in_ban_secs(),
            lock_in_pick_secs: default_lock_in_pick_secs(),
            preferences: LanePreferences {
                top: vec!["Darius".into(), "Garen".into(), "Malphite".into()],
                jungle: vec!["Vi".into(), "Warwick".into(), "Amumu".into()],
                mid: vec!["Lux".into(), "Ahri".into(), "Syndra".into()],
                bot: vec!["Jinx".into(), "Caitlyn".into(), "Jhin".into()],
                support: vec!["Thresh".into(), "Lulu".into(), "Sona".into()],
                fill: vec!["Garen".into(), "Lux".into()],
            },
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self> {
        let path = config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let cfg: Self = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            Ok(cfg)
        } else {
            let cfg = Self::default();
            cfg.save()?;
            tracing::info!("Created default config at {}", path.display());
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    /// Return the champion priority list for a given LCU position string.
    pub fn champions_for_position(&self, position: &str) -> &[String] {
        match position.to_lowercase().as_str() {
            "top" => &self.preferences.top,
            "jungle" => &self.preferences.jungle,
            "middle" | "mid" => &self.preferences.mid,
            "bottom" | "bot" | "adc" => &self.preferences.bot,
            "utility" | "support" => &self.preferences.support,
            _ => &self.preferences.fill,
        }
    }
}

fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}
