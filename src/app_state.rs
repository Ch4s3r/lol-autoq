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
        }
    }

    pub fn push_activity(self, msg: impl Into<String>, kind: ActivityKind) {
        let entry = ActivityEntry {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: msg.into(),
            kind,
        };
        let mut activities = self.activities;
        let mut log = activities.write();
        log.push_front(entry);
        if log.len() > 25 {
            log.pop_back();
        }
    }
}
