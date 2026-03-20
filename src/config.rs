use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.toml";

/// Sentinel value meaning "act immediately" (no timer check / no delay).
pub const INSTANT: u64 = u64::MAX;

/// Human-readable representation of a timer threshold or delay.
#[allow(dead_code)]
pub fn format_lock_in(secs: u64) -> String {
    if secs == INSTANT {
        "Instant".to_string()
    } else {
        format!("≤ {secs}s")
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
    /// Hover the pick champion when the timer has this many seconds or fewer remaining.
    /// INSTANT = hover as soon as the phase starts (default behaviour).
    #[serde(default = "default_hover_pick_secs")]
    pub hover_pick_secs: u64,
    /// Seconds to wait after a queue pop before accepting. INSTANT = accept immediately.
    #[serde(default = "default_accept_queue_delay_secs")]
    pub accept_queue_delay_secs: u64,
    /// Minimum random jitter added to each timer action (seconds). Default 0.
    #[serde(default)]
    pub jitter_min_secs: u64,
    /// Maximum random jitter added to each timer action (seconds). Default 0.
    #[serde(default)]
    pub jitter_max_secs: u64,
    /// Minimum log level written to the activity log and `lol-autoq.log`.
    /// Valid values: "error", "warn", "info", "debug", "trace". Default: "info".
    #[serde(default = "default_log_level")]
    pub log_level: String,
    pub preferences: LanePreferences,
}

fn default_lock_in_ban_secs() -> u64 {
    5
}

fn default_lock_in_pick_secs() -> u64 {
    10
}

fn default_hover_pick_secs() -> u64 {
    INSTANT
}

fn default_accept_queue_delay_secs() -> u64 {
    INSTANT
}

fn default_log_level() -> String {
    "info".to_string()
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
            hover_pick_secs: default_hover_pick_secs(),
            accept_queue_delay_secs: default_accept_queue_delay_secs(),
            jitter_min_secs: 0,
            jitter_max_secs: 0,
            log_level: default_log_level(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_lock_in ────────────────────────────────────────────────────────

    #[test]
    fn format_lock_in_instant_shows_label() {
        assert_eq!(format_lock_in(INSTANT), "Instant");
    }

    #[test]
    fn format_lock_in_numeric_shows_seconds() {
        assert_eq!(format_lock_in(5),  "≤ 5s");
        assert_eq!(format_lock_in(0),  "≤ 0s");
        assert_eq!(format_lock_in(30), "≤ 30s");
    }

    // ── champions_for_position ────────────────────────────────────────────────

    #[test]
    fn champions_for_position_routes_all_lanes() {
        let cfg = Config::default();
        assert!(!cfg.champions_for_position("top").is_empty());
        assert!(!cfg.champions_for_position("jungle").is_empty());
        assert!(!cfg.champions_for_position("middle").is_empty());
        assert!(!cfg.champions_for_position("mid").is_empty());
        assert!(!cfg.champions_for_position("bottom").is_empty());
        assert!(!cfg.champions_for_position("bot").is_empty());
        assert!(!cfg.champions_for_position("adc").is_empty());
        assert!(!cfg.champions_for_position("utility").is_empty());
        assert!(!cfg.champions_for_position("support").is_empty());
    }

    #[test]
    fn champions_for_position_unknown_falls_back_to_fill() {
        let cfg = Config::default();
        assert_eq!(
            cfg.champions_for_position("unknown"),
            cfg.preferences.fill.as_slice()
        );
    }

    #[test]
    fn champions_for_position_is_case_insensitive() {
        let cfg = Config::default();
        assert_eq!(
            cfg.champions_for_position("TOP"),
            cfg.champions_for_position("top")
        );
        assert_eq!(
            cfg.champions_for_position("JUNGLE"),
            cfg.champions_for_position("jungle")
        );
    }

    #[test]
    fn champions_for_position_mid_and_middle_are_same() {
        let cfg = Config::default();
        assert_eq!(
            cfg.champions_for_position("mid"),
            cfg.champions_for_position("middle")
        );
    }

    #[test]
    fn champions_for_position_bot_adc_bottom_are_same() {
        let cfg = Config::default();
        assert_eq!(cfg.champions_for_position("bot"),    cfg.champions_for_position("bottom"));
        assert_eq!(cfg.champions_for_position("adc"),    cfg.champions_for_position("bottom"));
    }

    #[test]
    fn champions_for_position_utility_and_support_are_same() {
        let cfg = Config::default();
        assert_eq!(
            cfg.champions_for_position("utility"),
            cfg.champions_for_position("support")
        );
    }
}
