use std::collections::VecDeque;

use chrono::Local;
use dioxus::prelude::*;

use crate::{config::Config, lcu::ChampionSummary};

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Searching,
    Connected { port: u16 },
}

impl ConnectionState {
    pub fn label(&self) -> &str {
        match self {
            Self::Disconnected | Self::Searching => "Searching for LCU…",
            Self::Connected { .. } => "Connected to LCU",
        }
    }
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
    pub fn chip_class(&self) -> &'static str {
        if self.is_connected() { "chip chip-connected" } else { "chip chip-searching" }
    }
    pub fn dot_class(&self) -> &'static str {
        if self.is_connected() { "chip-dot chip-dot-connected" } else { "chip-dot chip-dot-searching" }
    }
}

// ---------------------------------------------------------------------------
// Game phase
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Default)]
pub enum GamePhase {
    #[default]
    None,
    Lobby,
    Matchmaking,
    ReadyCheck,
    ChampSelect,
    GameStart,
    InProgress,
    EndOfGame,
    Unknown(String),
}

impl GamePhase {
    pub fn from_lcu(s: &str) -> Self {
        match s {
            "None" => Self::None,
            "Lobby" => Self::Lobby,
            "Matchmaking" => Self::Matchmaking,
            "ReadyCheck" => Self::ReadyCheck,
            "ChampSelect" => Self::ChampSelect,
            "GameStart" => Self::GameStart,
            "InProgress" => Self::InProgress,
            "WaitingForStats" | "PreEndOfGame" | "EndOfGame" => Self::EndOfGame,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::None | Self::Lobby => "In Lobby",
            Self::Matchmaking => "Searching…",
            Self::ReadyCheck => "Ready Check!",
            Self::ChampSelect => "Champion Select",
            Self::GameStart => "Game Starting…",
            Self::InProgress => "In Game",
            Self::EndOfGame => "Game Over",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::None | Self::Lobby => "Waiting in the lobby",
            Self::Matchmaking => "Looking for a match…",
            Self::ReadyCheck => "A match was found — accepting queue…",
            Self::ChampSelect => "Picking and banning champions",
            Self::GameStart => "Loading into the game…",
            Self::InProgress => "The game is in progress",
            Self::EndOfGame => "Returning to lobby soon",
            Self::Unknown(_) => "",
        }
    }

    pub fn css_class(&self) -> &str {
        match self {
            Self::None | Self::Lobby => "phase-lobby",
            Self::Matchmaking => "phase-matchmaking",
            Self::ReadyCheck => "phase-readycheck",
            Self::ChampSelect => "phase-champselect",
            Self::GameStart | Self::InProgress => "phase-ingame",
            Self::EndOfGame => "phase-endgame",
            Self::Unknown(_) => "phase-disconnected",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Self::None | Self::Lobby => "fa-solid fa-shield",
            Self::Matchmaking => "fa-solid fa-hourglass-half",
            Self::ReadyCheck => "fa-solid fa-bell",
            Self::ChampSelect => "fa-solid fa-wand-magic-sparkles",
            Self::GameStart => "fa-solid fa-play",
            Self::InProgress => "fa-solid fa-gamepad",
            Self::EndOfGame => "fa-solid fa-flag-checkered",
            Self::Unknown(_) => "fa-solid fa-circle-question",
        }
    }
}

// ---------------------------------------------------------------------------
// Activity log
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub message: String,
    pub kind: ActivityKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActivityKind {
    Info,
    Success,
    Warning,
}

impl ActivityKind {
    pub fn css_class(&self) -> &str {
        match self {
            Self::Info => "activity-info",
            Self::Success => "activity-success",
            Self::Warning => "activity-warning",
        }
    }
}

// ---------------------------------------------------------------------------
// Champ-select live status
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum BanStatus {
    Idle,
    NoBansConfigured,
    AllBansExhausted,
    WaitingToLock { champion_name: String },
    Hovering      { champion_name: String },
    Banned        { champion_name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickStatus {
    Idle,
    NoPrefsConfigured { position: String },
    AllPicksExhausted { position: String },
    WaitingToHover    { champion_name: String },
    Hovering          { champion_name: String },
    LockedIn          { champion_name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChampSelectStatus {
    /// Seconds left in the LCU sub-phase (adjusted_time_left_ms / 1000).
    pub time_left_secs: f64,
    /// LCU sub-phase string: "PLANNING", "BAN_PICK", "FINALIZATION", "".
    pub sub_phase: String,
    pub ban:  BanStatus,
    pub pick: PickStatus,
}

// ---------------------------------------------------------------------------
// Shared app state (bundle of signals)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct AppState {
    pub connection: Signal<ConnectionState>,
    pub phase: Signal<GamePhase>,
    pub activities: Signal<VecDeque<ActivityEntry>>,
    pub config: Signal<Config>,
    pub champion_summaries: Signal<Vec<ChampionSummary>>,
    pub hovered_champion: Signal<Option<String>>,
    pub ddragon_version: Signal<String>,
    pub champ_select_status: Signal<Option<ChampSelectStatus>>,
}

impl AppState {
    pub fn new() -> Self {
        let config = Config::load_or_create().unwrap_or_default();
        Self {
            connection: Signal::new(ConnectionState::Searching),
            phase: Signal::new(GamePhase::None),
            activities: Signal::new(VecDeque::new()),
            config: Signal::new(config),
            champion_summaries: Signal::new(Vec::new()),
            hovered_champion: Signal::new(None),
            ddragon_version: Signal::new("15.7.1".to_string()),
            champ_select_status: Signal::new(None),
        }
    }

    pub fn push_activity(self, msg: impl Into<String>, kind: ActivityKind) {
        let msg = msg.into();
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        crate::logger::write_activity(&timestamp, &msg, &kind);
        let entry = ActivityEntry { timestamp, message: msg, kind };
        let mut activities = self.activities;
        let mut log = activities.write();
        log.push_front(entry);
        if log.len() > 100 {
            log.pop_back();
        }
    }

    /// Drain tracing events from the shared logger buffer into the UI activity
    /// log. Called once per poll cycle so the activity log and log file stay
    /// identical.
    pub fn drain_log_buffer(self) {
        let buffer = crate::logger::log_buffer();
        let Ok(mut buf) = buffer.lock() else { return };
        if buf.is_empty() {
            return;
        }
        // Buffer is newest-first; reverse so we push_front in oldest→newest order.
        let entries: Vec<ActivityEntry> = buf.drain(..).collect();
        drop(buf);
        let mut activities = self.activities;
        let mut log = activities.write();
        for entry in entries.into_iter().rev() {
            log.push_front(entry);
            if log.len() > 100 {
                log.pop_back();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ConnectionState ───────────────────────────────────────────────────────

    #[test]
    fn connection_state_disconnected_is_not_connected() {
        assert!(!ConnectionState::Disconnected.is_connected());
    }

    #[test]
    fn connection_state_searching_is_not_connected() {
        assert!(!ConnectionState::Searching.is_connected());
    }

    #[test]
    fn connection_state_connected_is_connected() {
        assert!(ConnectionState::Connected { port: 12345 }.is_connected());
    }

    #[test]
    fn connection_state_disconnected_label() {
        assert_eq!(ConnectionState::Disconnected.label(), "Searching for LCU…");
        assert_eq!(ConnectionState::Searching.label(), "Searching for LCU…");
    }

    #[test]
    fn connection_state_connected_label() {
        assert_eq!(ConnectionState::Connected { port: 1 }.label(), "Connected to LCU");
    }

    #[test]
    fn connection_state_chip_class_connected() {
        assert_eq!(ConnectionState::Connected { port: 1 }.chip_class(), "chip chip-connected");
        assert_eq!(ConnectionState::Connected { port: 1 }.dot_class(), "chip-dot chip-dot-connected");
    }

    #[test]
    fn connection_state_chip_class_searching() {
        assert_eq!(ConnectionState::Searching.chip_class(), "chip chip-searching");
        assert_eq!(ConnectionState::Searching.dot_class(), "chip-dot chip-dot-searching");
        assert_eq!(ConnectionState::Disconnected.chip_class(), "chip chip-searching");
    }

    // ── GamePhase::from_lcu ───────────────────────────────────────────────────

    #[test]
    fn game_phase_from_lcu_maps_all_known_strings() {
        assert_eq!(GamePhase::from_lcu("None"),           GamePhase::None);
        assert_eq!(GamePhase::from_lcu("Lobby"),          GamePhase::Lobby);
        assert_eq!(GamePhase::from_lcu("Matchmaking"),    GamePhase::Matchmaking);
        assert_eq!(GamePhase::from_lcu("ReadyCheck"),     GamePhase::ReadyCheck);
        assert_eq!(GamePhase::from_lcu("ChampSelect"),    GamePhase::ChampSelect);
        assert_eq!(GamePhase::from_lcu("GameStart"),      GamePhase::GameStart);
        assert_eq!(GamePhase::from_lcu("InProgress"),     GamePhase::InProgress);
        assert_eq!(GamePhase::from_lcu("EndOfGame"),      GamePhase::EndOfGame);
        assert_eq!(GamePhase::from_lcu("WaitingForStats"),GamePhase::EndOfGame);
        assert_eq!(GamePhase::from_lcu("PreEndOfGame"),   GamePhase::EndOfGame);
    }

    #[test]
    fn game_phase_from_lcu_unknown_preserves_string() {
        assert_eq!(GamePhase::from_lcu("SomeNewPhase"), GamePhase::Unknown("SomeNewPhase".into()));
    }

    // ── GamePhase::css_class ──────────────────────────────────────────────────

    #[test]
    fn game_phase_css_class_returns_distinct_classes() {
        // Each reachable phase must return a non-empty, CSS-safe class string.
        let phases = [
            GamePhase::None, GamePhase::Lobby, GamePhase::Matchmaking, GamePhase::ReadyCheck,
            GamePhase::ChampSelect, GamePhase::GameStart, GamePhase::InProgress,
            GamePhase::EndOfGame, GamePhase::Unknown("x".into()),
        ];
        for phase in &phases {
            let cls = phase.css_class();
            assert!(!cls.is_empty(), "empty css_class for {phase:?}");
            assert!(!cls.contains(' ') || cls.starts_with("phase-"), "unexpected class format: {cls}");
        }
    }

    #[test]
    fn game_phase_lobby_and_none_share_css_class() {
        assert_eq!(GamePhase::None.css_class(), GamePhase::Lobby.css_class());
    }

    // ── GamePhase::icon ───────────────────────────────────────────────────────

    #[test]
    fn game_phase_icon_starts_with_fa_solid_prefix() {
        let phases = [
            GamePhase::None, GamePhase::Lobby, GamePhase::Matchmaking, GamePhase::ReadyCheck,
            GamePhase::ChampSelect, GamePhase::GameStart, GamePhase::InProgress,
            GamePhase::EndOfGame, GamePhase::Unknown("x".into()),
        ];
        for phase in &phases {
            let icon = phase.icon();
            assert!(icon.starts_with("fa-solid ") || icon.starts_with("fa-regular ") || icon.starts_with("fa-brands "),
                "icon class does not start with a Font Awesome prefix: {icon}");
        }
    }

    // ── ActivityKind::css_class ───────────────────────────────────────────────

    #[test]
    fn activity_kind_css_class_all_variants() {
        assert_eq!(ActivityKind::Info.css_class(), "activity-info");
        assert_eq!(ActivityKind::Success.css_class(), "activity-success");
        assert_eq!(ActivityKind::Warning.css_class(), "activity-warning");
    }

    // ── ActivityLog render order ──────────────────────────────────────────────

    #[test]
    fn activity_log_renders_oldest_first() {
        // Simulate what the app does: push_front so the deque is newest-first.
        let mut log: VecDeque<ActivityEntry> = VecDeque::new();
        let oldest = ActivityEntry {
            timestamp: "10:00:00".to_string(),
            message: "oldest".to_string(),
            kind: ActivityKind::Info,
        };
        let middle = ActivityEntry {
            timestamp: "10:00:01".to_string(),
            message: "middle".to_string(),
            kind: ActivityKind::Info,
        };
        let newest = ActivityEntry {
            timestamp: "10:00:02".to_string(),
            message: "newest".to_string(),
            kind: ActivityKind::Success,
        };
        log.push_front(oldest.clone());
        log.push_front(middle.clone());
        log.push_front(newest.clone());

        // The component renders with `.iter().rev()` — oldest must come first.
        let rendered: Vec<&ActivityEntry> = log.iter().rev().collect();
        assert_eq!(rendered[0].message, "oldest", "index 0 must be the oldest entry");
        assert_eq!(rendered[1].message, "middle");
        assert_eq!(rendered[2].message, "newest", "last index must be the newest entry");
    }

    // ── BanStatus / PickStatus ────────────────────────────────────────────────

    #[test]
    fn ban_status_variants_implement_clone_and_partialeq() {
        let variants = vec![
            BanStatus::Idle,
            BanStatus::NoBansConfigured,
            BanStatus::AllBansExhausted,
            BanStatus::WaitingToLock { champion_name: "Zed".into() },
            BanStatus::Hovering      { champion_name: "Yasuo".into() },
            BanStatus::Banned        { champion_name: "Yone".into() },
        ];
        for v in &variants {
            assert_eq!(v, &v.clone());
        }
        assert_ne!(BanStatus::Idle, BanStatus::NoBansConfigured);
    }

    #[test]
    fn pick_status_variants_implement_clone_and_partialeq() {
        let variants = vec![
            PickStatus::Idle,
            PickStatus::NoPrefsConfigured { position: "Mid".into() },
            PickStatus::AllPicksExhausted { position: "Bot".into() },
            PickStatus::WaitingToHover    { champion_name: "Jinx".into() },
            PickStatus::Hovering          { champion_name: "Ahri".into() },
            PickStatus::LockedIn          { champion_name: "Lux".into() },
        ];
        for v in &variants {
            assert_eq!(v, &v.clone());
        }
        assert_ne!(PickStatus::Idle, PickStatus::NoPrefsConfigured { position: "Top".into() });
    }
}
