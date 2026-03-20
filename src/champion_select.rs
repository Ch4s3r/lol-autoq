use std::collections::{HashMap, HashSet};

use anyhow::Result;
use tracing::{info, trace, warn};

use crate::config::{Config, INSTANT};
use crate::lcu::{Action, ChampSelectSession, ChampionSummary, LcuClient};

/// Compute the effective lock-in threshold in seconds after applying jitter.
/// Jitter is added to the configured threshold so the bot acts later (more human).
pub(crate) fn effective_threshold(configured_secs: u64, jitter_secs: u64) -> f64 {
    (configured_secs + jitter_secs) as f64
}

/// Returns:
/// - `lookup`  : lowercase name/alias → champion id  (for resolving config preferences)
/// - `display` : champion id → proper display name    (for human-readable log messages)
pub fn build_champion_map(
    summaries: &[ChampionSummary],
) -> (HashMap<String, i64>, HashMap<i64, String>) {
    let valid: Vec<_> = summaries.iter().filter(|c| c.is_playable()).collect();
    let mut lookup = HashMap::with_capacity(valid.len() * 2);
    let mut display = HashMap::with_capacity(valid.len());
    for c in valid {
        lookup.insert(c.name.to_lowercase(), c.id);
        lookup.insert(c.alias.to_lowercase(), c.id);
        display.insert(c.id, c.name.clone());
    }
    (lookup, display)
}

fn friendly_position(pos: &str) -> &'static str {
    match pos.to_ascii_lowercase().as_str() {
        "top"                    => "Top",
        "jungle"                 => "Jungle",
        "middle" | "mid"         => "Mid",
        "bottom" | "bot" | "adc" => "Bot",
        "utility" | "support"    => "Support",
        "fill"                   => "Fill",
        _                        => "Unknown",
    }
}

/// Return the single ban action that belongs to `local_player_cell_id`
/// and is currently in-progress and not yet completed.
pub fn find_active_ban_action(session: &ChampSelectSession) -> Option<&Action> {
    session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == session.local_player_cell_id
            && a.action_type == "ban"
            && a.is_in_progress
            && !a.completed
    })
}

/// Find the best available ban target from the config preference list.
fn best_ban_target(
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    already_banned: &HashSet<i64>,
    teammate_ban_hovers: &HashSet<i64>,
) -> Option<(i64, String)> {
    config.bans.iter().find_map(|pref_name| {
        let key = pref_name.to_ascii_lowercase();
        match champion_map.get(&key) {
            None => {
                trace!(champion = %pref_name, "ban target not found in game data — check spelling");
                None
            }
            Some(&id) if already_banned.contains(&id) => {
                trace!(champion = %pref_name, "already banned — skipping");
                None
            }
            Some(&id) if teammate_ban_hovers.contains(&id) => {
                trace!(champion = %pref_name, "teammate is already banning this — skipping");
                None
            }
            Some(&id) => {
                let name = display_names
                    .get(&id)
                    .map(String::as_str)
                    .unwrap_or(pref_name)
                    .to_owned();
                Some((id, name))
            }
        }
    })
}

// ── Ban-phase decision ─────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum BanDecision {
    /// No active ban action, wrong phase, etc.
    Idle,
    /// Config has an empty bans list.
    NoBansConfigured,
    /// Every preferred ban is already banned.
    AllBansExhausted,
    /// Hover this champion immediately.
    Hover { action_id: i64, champion_id: i64, champion_name: String },
    /// Already hovered; waiting for the lock-in window.
    WaitForTimer { champion_name: String, remaining_secs: f64, threshold_secs: f64 },
    /// Time to lock in.
    LockIn { action_id: i64, champion_id: i64, champion_name: String, remaining_secs: f64 },
}

/// Pure decision function for the ban phase — no I/O, fully unit-testable.
pub fn decide_ban(
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_ban: Option<i64>,
    lock_in_jitter: u64,
) -> BanDecision {
    let action = match find_active_ban_action(session) {
        Some(a) => a,
        None => return BanDecision::Idle,
    };

    // Only act during the actual ban phase. During "PLANNING" the ban action
    // is already marked is_in_progress but it's too early to hover or lock.
    let phase = session.timer.phase.as_str();
    if phase == "PLANNING" || phase.is_empty() {
        return BanDecision::Idle;
    }

    if config.bans.is_empty() {
        return BanDecision::NoBansConfigured;
    }

    let already_banned: HashSet<i64> = session
        .bans
        .my_team_bans
        .iter()
        .chain(session.bans.their_team_bans.iter())
        .copied()
        .collect();

    // Champions another teammate has already hovered for banning should be
    // skipped so we don't duplicate their intent.
    let teammate_ban_hovers: HashSet<i64> = session
        .actions
        .iter()
        .flatten()
        .filter(|a| {
            a.action_type == "ban"
                && a.actor_cell_id != session.local_player_cell_id
                && a.champion_id != 0
        })
        .map(|a| a.champion_id)
        .collect();

    let (chosen_id, chosen_name) = match best_ban_target(config, champion_map, display_names, &already_banned, &teammate_ban_hovers) {
        Some(pair) => pair,
        None => return BanDecision::AllBansExhausted,
    };

    if hovered_ban != Some(chosen_id) {
        return BanDecision::Hover {
            action_id: action.id,
            champion_id: chosen_id,
            champion_name: chosen_name,
        };
    }

    let remaining_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
    if config.lock_in_ban_secs != INSTANT {
        let threshold = effective_threshold(config.lock_in_ban_secs, lock_in_jitter);
        if remaining_secs > threshold {
            return BanDecision::WaitForTimer {
                champion_name: chosen_name,
                remaining_secs,
                threshold_secs: threshold,
            };
        }
    }

    BanDecision::LockIn {
        action_id: action.id,
        champion_id: chosen_id,
        champion_name: chosen_name,
        remaining_secs,
    }
}

/// Ban phase handler — thin dispatcher that executes the `decide_ban` decision via LCU calls.
pub async fn handle_ban_phase(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_ban: &mut Option<i64>,
    lock_in_jitter: u64,
) -> Result<bool> {
    match decide_ban(session, config, champion_map, display_names, *hovered_ban, lock_in_jitter) {
        BanDecision::Idle => Ok(false),
        BanDecision::NoBansConfigured => {
            warn!("no ban preferences configured — add some via `lol-autoq configure`");
            Ok(false)
        }
        BanDecision::AllBansExhausted => {
            warn!("All preferred bans are already banned — add more options");
            Ok(false)
        }
        BanDecision::Hover { action_id, champion_id, champion_name } => {
            info!(champion = %champion_name, "Hovering ban...");
            client.hover_champion(action_id, champion_id).await?;
            *hovered_ban = Some(champion_id);
            Ok(false)
        }
        BanDecision::WaitForTimer { champion_name, remaining_secs, threshold_secs } => {
            trace!(
                remaining = format!("{remaining_secs:.1}s"),
                threshold = format!("{threshold_secs:.0}s"),
                champion = %champion_name,
                "waiting to lock ban"
            );
            Ok(false)
        }
        BanDecision::LockIn { action_id, champion_id, champion_name, remaining_secs } => {
            info!(
                champion = %champion_name,
                remaining = format!("{remaining_secs:.1}s"),
                ban_order = %config.bans.join(" -> "),
                "Locking in ban!"
            );
            client.lock_champion(action_id, champion_id).await?;
            info!(champion = %champion_name, "Ban complete!");
            Ok(true)
        }
    }
}

/// Return our pick action regardless of whether it is in-progress yet.
pub fn find_pick_action(session: &ChampSelectSession) -> Option<&Action> {
    session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == session.local_player_cell_id
            && a.action_type == "pick"
            && !a.completed
    })
}

/// Returns true when every ban action in the session has been completed,
/// i.e. the ban phase is fully over and actual picks are happening.
fn all_bans_completed(session: &ChampSelectSession) -> bool {
    session
        .actions
        .iter()
        .flatten()
        .filter(|a| a.action_type == "ban")
        .all(|a| a.completed)
}

/// into a HashSet for O(1) membership testing.
fn unavailable_champion_ids(session: &ChampSelectSession) -> HashSet<i64> {
    let enemy_picks = session
        .actions
        .iter()
        .flatten()
        .filter(|a| {
            a.action_type == "pick"
                && a.completed
                && a.actor_cell_id != session.local_player_cell_id
                && a.champion_id != 0
        })
        .map(|a| a.champion_id);

    session
        .my_team
        .iter()
        .filter(|m| m.champion_id != 0)
        .map(|m| m.champion_id)
        .chain(session.bans.my_team_bans.iter().copied())
        .chain(session.bans.their_team_bans.iter().copied())
        .chain(enemy_picks)
        .collect()
}

/// Determine our assigned lane from the session.
pub fn local_assigned_position(session: &ChampSelectSession) -> &str {
    session
        .my_team
        .iter()
        .find(|m| m.cell_id == session.local_player_cell_id)
        .map(|m| m.assigned_position.as_str())
        .unwrap_or("")
}

/// Find the best available champion pick from the preference list.
fn best_pick_target(
    config: &Config,
    raw_position: &str,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    unavailable: &HashSet<i64>,
) -> Option<(i64, String)> {
    let prefs = config.champions_for_position(raw_position);
    prefs.iter().find_map(|pref_name| {
        let key = pref_name.to_ascii_lowercase();
        match champion_map.get(&key) {
            None => {
                trace!(champion = %pref_name, "not found in game data — check spelling in config.toml");
                None
            }
            Some(&id) if unavailable.contains(&id) => {
                trace!(champion = %pref_name, "banned or already picked — skipping");
                None
            }
            Some(&id) => {
                let name = display_names
                    .get(&id)
                    .map(String::as_str)
                    .unwrap_or(pref_name)
                    .to_owned();
                Some((id, name))
            }
        }
    })
}

// ── Pick-phase decision ────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum PickDecision {
    /// No pick action found.
    Idle,
    /// Config has no preferences for this position.
    NoPrefsConfigured { position: String },
    /// Every preferred champion is banned or already picked.
    AllPicksExhausted { position: String },
    /// Hover timer hasn't elapsed yet.
    WaitForHoverTimer { champion_name: String, remaining_secs: f64, threshold_secs: f64 },
    /// Hover this champion now.
    Hover { action_id: i64, champion_id: i64, champion_name: String, position: String },
    /// Hovered; waiting for our turn or the lock-in window.
    WaitForLockIn,
    /// Hovered champion got banned — re-evaluate next cycle.
    StaleHover { champion_name: String },
    /// Lock in.
    LockIn { action_id: i64, champion_id: i64, champion_name: String, remaining_secs: f64, position: String },
}

/// Pure decision function for the pick phase — no I/O, fully unit-testable.
pub fn decide_pick(
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_pick: Option<(i64, i64)>,
    hover_jitter: u64,
    pick_jitter: u64,
) -> PickDecision {
    let action = match find_pick_action(session) {
        Some(a) => a,
        None => return PickDecision::Idle,
    };

    let raw_position = local_assigned_position(session);
    let position_label = friendly_position(raw_position).to_owned();

    let prefs = config.champions_for_position(raw_position);
    if prefs.is_empty() {
        return PickDecision::NoPrefsConfigured { position: position_label };
    }

    let unavailable = unavailable_champion_ids(session);
    trace!(ids = ?unavailable, "unavailable champions");

    let (chosen_id, chosen_name) = match best_pick_target(config, raw_position, champion_map, display_names, &unavailable) {
        Some(pair) => pair,
        None => return PickDecision::AllPicksExhausted { position: position_label },
    };

    if hovered_pick != Some((action.id, chosen_id)) {
        let remaining_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
        if config.hover_pick_secs != INSTANT {
            let threshold = effective_threshold(config.hover_pick_secs, hover_jitter);
            if remaining_secs > threshold {
                return PickDecision::WaitForHoverTimer {
                    champion_name: chosen_name,
                    remaining_secs,
                    threshold_secs: threshold,
                };
            }
        }
        return PickDecision::Hover {
            action_id: action.id,
            champion_id: chosen_id,
            champion_name: chosen_name,
            position: position_label,
        };
    }

    // Only lock in when it's actually our turn in the pick phase
    // (not during the simultaneous intent phase at the start).
    if !action.is_in_progress || !all_bans_completed(session) {
        return PickDecision::WaitForLockIn;
    }

    let remaining_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
    if config.lock_in_pick_secs != INSTANT {
        let threshold = effective_threshold(config.lock_in_pick_secs, pick_jitter);
        if remaining_secs > threshold {
            return PickDecision::WaitForLockIn;
        }
    }

    // Re-check availability right before locking (champion could have been
    // banned between the hover and now).
    let unavailable = unavailable_champion_ids(session);
    if unavailable.contains(&chosen_id) {
        return PickDecision::StaleHover { champion_name: chosen_name };
    }

    PickDecision::LockIn {
        action_id: action.id,
        champion_id: chosen_id,
        champion_name: chosen_name,
        remaining_secs,
        position: position_label,
    }
}

/// Core champion-select handler — thin dispatcher that executes the `decide_pick` decision.
#[allow(clippy::too_many_arguments)]
pub async fn handle_champion_select(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_pick: &mut Option<(i64, i64)>,
    hover_jitter: u64,
    pick_jitter: u64,
) -> Result<bool> {
    let prefs_for_log = config.champions_for_position(local_assigned_position(session)).join(" -> ");
    match decide_pick(session, config, champion_map, display_names, *hovered_pick, hover_jitter, pick_jitter) {
        PickDecision::Idle => Ok(false),
        PickDecision::NoPrefsConfigured { position } => {
            warn!(position = %position, "no champion preferences configured — please edit config.toml");
            Ok(false)
        }
        PickDecision::AllPicksExhausted { position } => {
            warn!(position = %position, "All preferred champions are banned/picked — add more options to config.toml");
            Ok(false)
        }
        PickDecision::WaitForHoverTimer { champion_name, remaining_secs, threshold_secs } => {
            trace!(
                remaining = format!("{remaining_secs:.1}s"),
                threshold = format!("{threshold_secs:.0}s"),
                champion = %champion_name,
                "waiting to hover champion"
            );
            Ok(false)
        }
        PickDecision::Hover { action_id, champion_id, champion_name, position } => {
            info!(
                position = %position,
                champion = %champion_name,
                pick_order = %prefs_for_log,
                "Hovering champion..."
            );
            client.hover_champion(action_id, champion_id).await?;
            *hovered_pick = Some((action_id, champion_id));
            Ok(false)
        }
        PickDecision::WaitForLockIn => Ok(false),
        PickDecision::StaleHover { champion_name } => {
            warn!(champion = %champion_name, "champion was banned since hovering — switching");
            *hovered_pick = None;
            Ok(false)
        }
        PickDecision::LockIn { action_id, champion_id, champion_name, remaining_secs, position } => {
            info!(
                position = %position,
                champion = %champion_name,
                remaining = format!("{remaining_secs:.1}s"),
                "Locking in champion!"
            );
            client.lock_champion(action_id, champion_id).await?;
            info!(champion = %champion_name, "Lock-in complete!");
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::config::Config;
    use crate::lcu::{Action, Bans, ChampSelectSession, ChampionSummary, PhaseTimer, TeamMember};

    // ── fixtures ─────────────────────────────────────────────────────────────

    fn make_summaries() -> Vec<ChampionSummary> {
        vec![
            ChampionSummary {
                id: 1,
                name: "Ahri".into(),
                alias: "Ahri".into(),
                square_portrait_path: "/champs/Ahri.png".into(),
            },
            ChampionSummary {
                id: 2,
                name: "Zed".into(),
                alias: "Zed".into(),
                square_portrait_path: "/champs/Zed.png".into(),
            },
            // non-playable — must be filtered out
            ChampionSummary {
                id: -1,
                name: "Bot".into(),
                alias: "Bot".into(),
                square_portrait_path: "/-1.png".into(),
            },
        ]
    }

    fn make_action(id: i64, cell_id: i64, action_type: &str, in_progress: bool, completed: bool) -> Action {
        Action {
            id,
            actor_cell_id: cell_id,
            action_type: action_type.into(),
            is_in_progress: in_progress,
            completed,
            champion_id: 0,
        }
    }

    fn make_session(
        local_cell_id: i64,
        actions: Vec<Vec<Action>>,
        my_team: Vec<TeamMember>,
        bans: Bans,
        phase: &str,
    ) -> ChampSelectSession {
        ChampSelectSession {
            local_player_cell_id: local_cell_id,
            timer: PhaseTimer {
                adjusted_time_left_ms: 10_000,
                total_time_ms: 30_000,
                phase: phase.into(),
            },
            actions,
            my_team,
            bans,
        }
    }

    fn make_member(cell_id: i64, position: &str, champion_id: i64) -> TeamMember {
        TeamMember {
            cell_id,
            assigned_position: position.into(),
            champion_id,
            summoner_id: 0,
        }
    }

    // ── effective_threshold ───────────────────────────────────────────────────

    #[test]
    fn effective_threshold_no_jitter_keeps_threshold() {
        assert_eq!(effective_threshold(10, 0), 10.0);
    }

    #[test]
    fn effective_threshold_jitter_increases_threshold() {
        assert_eq!(effective_threshold(10, 3), 13.0);
    }

    #[test]
    fn effective_threshold_zero_base_with_jitter() {
        assert_eq!(effective_threshold(0, 5), 5.0);
    }

    #[test]
    fn effective_threshold_both_zero_stays_zero() {
        assert_eq!(effective_threshold(0, 0), 0.0);
    }

    // ── build_champion_map ────────────────────────────────────────────────────

    #[test]
    fn build_champion_map_resolves_by_name() {
        let (lookup, _) = build_champion_map(&make_summaries());
        assert_eq!(lookup.get("ahri"), Some(&1));
    }

    #[test]
    fn build_champion_map_resolves_by_alias() {
        let (lookup, _) = build_champion_map(&make_summaries());
        assert_eq!(lookup.get("zed"), Some(&2));
    }

    #[test]
    fn build_champion_map_filters_non_playable() {
        let (lookup, _) = build_champion_map(&make_summaries());
        assert_eq!(lookup.get("bot"), None);
    }

    #[test]
    fn build_champion_map_display_names_populated() {
        let (_, display) = build_champion_map(&make_summaries());
        assert_eq!(display.get(&1), Some(&"Ahri".to_string()));
        assert_eq!(display.get(&2), Some(&"Zed".to_string()));
    }

    // ── friendly_position ─────────────────────────────────────────────────────

    #[test]
    fn friendly_position_maps_all_known_aliases() {
        assert_eq!(friendly_position("top"),     "Top");
        assert_eq!(friendly_position("jungle"),  "Jungle");
        assert_eq!(friendly_position("middle"),  "Mid");
        assert_eq!(friendly_position("mid"),     "Mid");
        assert_eq!(friendly_position("bottom"),  "Bot");
        assert_eq!(friendly_position("bot"),     "Bot");
        assert_eq!(friendly_position("adc"),     "Bot");
        assert_eq!(friendly_position("utility"), "Support");
        assert_eq!(friendly_position("support"), "Support");
        assert_eq!(friendly_position("fill"),    "Fill");
    }

    #[test]
    fn friendly_position_unknown_returns_unknown() {
        assert_eq!(friendly_position(""),        "Unknown");
        assert_eq!(friendly_position("TOPLANE"), "Unknown");
    }

    // ── find_active_ban_action ────────────────────────────────────────────────

    #[test]
    fn find_active_ban_action_returns_matching_ban() {
        let action = make_action(7, 3, "ban", true, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert_eq!(find_active_ban_action(&session).map(|a| a.id), Some(7));
    }

    #[test]
    fn find_active_ban_action_ignores_completed_actions() {
        let action = make_action(7, 3, "ban", true, true);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_active_ban_action(&session).is_none());
    }

    #[test]
    fn find_active_ban_action_ignores_wrong_cell_id() {
        let action = make_action(7, 99, "ban", true, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_active_ban_action(&session).is_none());
    }

    #[test]
    fn find_active_ban_action_ignores_pick_actions() {
        let action = make_action(7, 3, "pick", true, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_active_ban_action(&session).is_none());
    }

    #[test]
    fn find_active_ban_action_ignores_not_in_progress() {
        let action = make_action(7, 3, "ban", false, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_active_ban_action(&session).is_none());
    }

    // ── find_pick_action ──────────────────────────────────────────────────────

    #[test]
    fn find_pick_action_returns_pick_regardless_of_in_progress() {
        let action = make_action(5, 3, "pick", false, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_pick_action(&session).is_some());
    }

    #[test]
    fn find_pick_action_ignores_completed_picks() {
        let action = make_action(5, 3, "pick", true, true);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_pick_action(&session).is_none());
    }

    #[test]
    fn find_pick_action_ignores_ban_actions() {
        let action = make_action(5, 3, "ban", true, false);
        let session = make_session(3, vec![vec![action]], vec![], Bans::default(), "BAN_PICK");
        assert!(find_pick_action(&session).is_none());
    }

    // ── all_bans_completed ────────────────────────────────────────────────────

    #[test]
    fn all_bans_completed_true_when_all_done() {
        let actions = vec![vec![
            make_action(1, 0, "ban", false, true),
            make_action(2, 0, "ban", false, true),
        ]];
        let session = make_session(0, actions, vec![], Bans::default(), "BAN_PICK");
        assert!(all_bans_completed(&session));
    }

    #[test]
    fn all_bans_completed_false_when_any_pending() {
        let actions = vec![vec![
            make_action(1, 0, "ban", false, true),
            make_action(2, 0, "ban", true, false),
        ]];
        let session = make_session(0, actions, vec![], Bans::default(), "BAN_PICK");
        assert!(!all_bans_completed(&session));
    }

    #[test]
    fn all_bans_completed_true_when_no_ban_actions_exist() {
        let session = make_session(0, vec![], vec![], Bans::default(), "BAN_PICK");
        assert!(all_bans_completed(&session));
    }

    // ── unavailable_champion_ids ──────────────────────────────────────────────

    #[test]
    fn unavailable_champion_ids_includes_picked_teammates() {
        let member = make_member(1, "top", 42);
        let session = make_session(0, vec![], vec![member], Bans::default(), "BAN_PICK");
        assert!(unavailable_champion_ids(&session).contains(&42));
    }

    #[test]
    fn unavailable_champion_ids_excludes_zero_champion_ids() {
        let member = make_member(1, "top", 0); // no champion selected yet
        let session = make_session(0, vec![], vec![member], Bans::default(), "BAN_PICK");
        assert!(!unavailable_champion_ids(&session).contains(&0));
    }

    #[test]
    fn unavailable_champion_ids_includes_all_bans() {
        let bans = Bans { my_team_bans: vec![10], their_team_bans: vec![20] };
        let session = make_session(0, vec![], vec![], bans, "BAN_PICK");
        let ids = unavailable_champion_ids(&session);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
    }

    // ── local_assigned_position ───────────────────────────────────────────────

    #[test]
    fn local_assigned_position_returns_correct_position() {
        let member = make_member(3, "jungle", 0);
        let session = make_session(3, vec![], vec![member], Bans::default(), "BAN_PICK");
        assert_eq!(local_assigned_position(&session), "jungle");
    }

    #[test]
    fn local_assigned_position_returns_empty_when_not_found() {
        let member = make_member(99, "jungle", 0);
        let session = make_session(3, vec![], vec![member], Bans::default(), "BAN_PICK");
        assert_eq!(local_assigned_position(&session), "");
    }

    // ── decide_ban ────────────────────────────────────────────────────────────

    fn session_with_ban_action(cell_id: i64, phase: &str, timer_ms: i64) -> ChampSelectSession {
        let action = make_action(10, cell_id, "ban", true, false);
        let mut s = make_session(cell_id, vec![vec![action]], vec![], Bans::default(), phase);
        s.timer.adjusted_time_left_ms = timer_ms;
        s
    }

    fn default_ban_config() -> Config {
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into(), "Zed".into()];
        cfg.lock_in_ban_secs = 5;
        cfg
    }

    #[test]
    fn decide_ban_idle_when_no_active_ban_action() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = make_session(3, vec![], vec![], Bans::default(), "BAN_PICK");
        assert_eq!(decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0), BanDecision::Idle);
    }

    #[test]
    fn decide_ban_idle_during_planning_phase() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "PLANNING", 20_000);
        assert_eq!(decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0), BanDecision::Idle);
    }

    #[test]
    fn decide_ban_no_bans_configured() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 20_000);
        let mut cfg = Config::default();
        cfg.bans = vec![];
        assert_eq!(decide_ban(&session, &cfg, &lookup, &display, None, 0), BanDecision::NoBansConfigured);
    }

    #[test]
    fn decide_ban_all_bans_exhausted() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 20_000);
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into()];
        // Ahri already banned
        let mut s = session;
        s.bans.my_team_bans = vec![1];
        assert_eq!(decide_ban(&s, &cfg, &lookup, &display, None, 0), BanDecision::AllBansExhausted);
    }

    #[test]
    fn decide_ban_hover_when_not_yet_hovered() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 20_000);
        let result = decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0);
        assert_eq!(result, BanDecision::Hover { action_id: 10, champion_id: 1, champion_name: "Ahri".into() });
    }

    #[test]
    fn decide_ban_wait_for_timer_when_hovered_and_time_remaining() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 20_000); // 20s remaining
        let mut cfg = default_ban_config();
        cfg.lock_in_ban_secs = 5; // threshold = 5s
        let result = decide_ban(&session, &cfg, &lookup, &display, Some(1), 0);
        assert!(matches!(result, BanDecision::WaitForTimer { .. }));
    }

    #[test]
    fn decide_ban_lock_in_when_timer_at_threshold() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 3_000); // 3s remaining
        let mut cfg = default_ban_config();
        cfg.lock_in_ban_secs = 5; // threshold 5s → 3s <= 5s → lock in
        let result = decide_ban(&session, &cfg, &lookup, &display, Some(1), 0);
        assert_eq!(result, BanDecision::LockIn {
            action_id: 10,
            champion_id: 1,
            champion_name: "Ahri".into(),
            remaining_secs: 3.0,
        });
    }

    #[test]
    fn decide_ban_instant_skips_timer_check() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_ban_action(3, "BAN_PICK", 999_000); // huge time remaining
        let mut cfg = default_ban_config();
        cfg.lock_in_ban_secs = crate::config::INSTANT;
        let result = decide_ban(&session, &cfg, &lookup, &display, Some(1), 0);
        assert!(matches!(result, BanDecision::LockIn { .. }));
    }

    // ── decide_pick ───────────────────────────────────────────────────────────

    fn session_with_pick_action(
        cell_id: i64,
        position: &str,
        is_in_progress: bool,
        timer_ms: i64,
        bans: Bans,
    ) -> ChampSelectSession {
        let pick = make_action(20, cell_id, "pick", is_in_progress, false);
        // all bans already completed so the lock-in guard passes
        let ban = make_action(1, 0, "ban", false, true);
        let member = make_member(cell_id, position, 0);
        let mut s = make_session(cell_id, vec![vec![ban, pick]], vec![member], bans, "BAN_PICK");
        s.timer.adjusted_time_left_ms = timer_ms;
        s
    }

    fn default_pick_config() -> Config {
        let mut cfg = Config::default();
        cfg.preferences.mid = vec!["Ahri".into(), "Zed".into()];
        cfg.hover_pick_secs = crate::config::INSTANT;
        cfg.lock_in_pick_secs = 10;
        cfg
    }

    #[test]
    fn decide_pick_idle_when_no_pick_action() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = make_session(3, vec![], vec![], Bans::default(), "BAN_PICK");
        assert_eq!(decide_pick(&session, &default_pick_config(), &lookup, &display, None, 0, 0), PickDecision::Idle);
    }

    #[test]
    fn decide_pick_hover_immediately_when_not_yet_hovered() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_pick_action(3, "middle", false, 20_000, Bans::default());
        let result = decide_pick(&session, &default_pick_config(), &lookup, &display, None, 0, 0);
        assert_eq!(result, PickDecision::Hover {
            action_id: 20,
            champion_id: 1,
            champion_name: "Ahri".into(),
            position: "Mid".into(),
        });
    }

    #[test]
    fn decide_pick_wait_for_hover_timer_when_not_elapsed() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_pick_action(3, "middle", false, 30_000, Bans::default());
        let mut cfg = default_pick_config();
        cfg.hover_pick_secs = 5; // hover only when <= 5s remain; 30s > 5s → wait
        let result = decide_pick(&session, &cfg, &lookup, &display, None, 0, 0);
        assert!(matches!(result, PickDecision::WaitForHoverTimer { .. }));
    }

    #[test]
    fn decide_pick_wait_for_lock_in_when_not_our_turn() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // is_in_progress = false → not our turn yet
        let session = session_with_pick_action(3, "middle", false, 20_000, Bans::default());
        let result = decide_pick(&session, &default_pick_config(), &lookup, &display, Some((20, 1)), 0, 0);
        assert_eq!(result, PickDecision::WaitForLockIn);
    }

    #[test]
    fn decide_pick_wait_for_lock_in_when_timer_not_reached() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_pick_action(3, "middle", true, 20_000, Bans::default());
        let mut cfg = default_pick_config();
        cfg.lock_in_pick_secs = 5; // 20s remaining > 5s threshold
        let result = decide_pick(&session, &cfg, &lookup, &display, Some((20, 1)), 0, 0);
        assert_eq!(result, PickDecision::WaitForLockIn);
    }

    #[test]
    fn decide_pick_lock_in_when_timer_reached() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_pick_action(3, "middle", true, 3_000, Bans::default());
        let mut cfg = default_pick_config();
        cfg.lock_in_pick_secs = 5; // 3s <= 5s → lock in
        let result = decide_pick(&session, &cfg, &lookup, &display, Some((20, 1)), 0, 0);
        assert_eq!(result, PickDecision::LockIn {
            action_id: 20,
            champion_id: 1,
            champion_name: "Ahri".into(),
            remaining_secs: 3.0,
            position: "Mid".into(),
        });
    }

    #[test]
    fn decide_pick_stale_hover_when_chosen_champion_banned_before_lock_in() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // Ahri (id=1) gets banned between hover and lock-in
        let bans = Bans { my_team_bans: vec![1], their_team_bans: vec![] };
        let session = session_with_pick_action(3, "middle", true, 3_000, bans);
        let mut cfg = default_pick_config();
        cfg.preferences.mid = vec!["Ahri".into()]; // only Ahri configured, no fallback
        cfg.lock_in_pick_secs = 5;
        // We're hovering Ahri but it got banned → AllPicksExhausted (best_pick_target returns None).
        // StaleHover would require Ahri to pass best_pick_target (not in unavailable) but then fail
        // the secondary unavailability check just before lock-in. Both checks derive unavailable from
        // the same live session state, so they are always consistent — StaleHover is unreachable.
        // The enemy-pick scenario (hovered champion taken by an opponent) is covered separately in
        // decide_pick_re_hovers_when_hovered_champion_picked_by_enemy, where unavailable_champion_ids
        // now includes completed non-local pick actions, causing best_pick_target to skip Ahri and
        // return Zed, which triggers a re-hover before we even reach the StaleHover check.
        let result = decide_pick(&session, &cfg, &lookup, &display, Some((20, 1)), 0, 0);
        assert_eq!(result, PickDecision::AllPicksExhausted { position: "Mid".into() });
    }

    #[test]
    fn decide_pick_re_hovers_when_hovered_champion_picked_by_enemy() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // Local player is cell 3, not-yet-our-turn pick action (is_in_progress=false)
        let our_pick   = make_action(20, 3, "pick", false, false);
        let ban        = make_action(1, 0, "ban", false, true); // completed ban so guard passes
        let member     = make_member(3, "middle", 0);
        // Enemy (cell 7, not local cell 3) has already completed a pick of Ahri (id=1)
        let enemy_pick = make_action_with_champ(30, 7, "pick", false, true, 1);
        let mut session = make_session(
            3,
            vec![vec![ban, our_pick, enemy_pick]],
            vec![member],
            Bans::default(),
            "BAN_PICK",
        );
        session.timer.adjusted_time_left_ms = 20_000;
        // Config: primary = Ahri, secondary = Zed
        let cfg = default_pick_config(); // mid = ["Ahri", "Zed"], hover_pick_secs = INSTANT
        // We previously hovered Ahri (action_id=20, champion_id=1)
        let result = decide_pick(&session, &cfg, &lookup, &display, Some((20, 1)), 0, 0);
        // Ahri is now unavailable (picked by enemy) → bot should re-hover Zed
        assert_eq!(result, PickDecision::Hover {
            action_id: 20,
            champion_id: 2,
            champion_name: "Zed".into(),
            position: "Mid".into(),
        });
    }

    #[test]
    fn decide_pick_all_picks_exhausted_when_every_champion_banned() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let bans = Bans { my_team_bans: vec![1, 2], their_team_bans: vec![] };
        let session = session_with_pick_action(3, "middle", true, 3_000, bans);
        let result = decide_pick(&session, &default_pick_config(), &lookup, &display, None, 0, 0);
        assert_eq!(result, PickDecision::AllPicksExhausted { position: "Mid".into() });
    }

    #[test]
    fn decide_pick_instant_lock_skips_timer_check() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let session = session_with_pick_action(3, "middle", true, 999_000, Bans::default());
        let mut cfg = default_pick_config();
        cfg.lock_in_pick_secs = crate::config::INSTANT;
        let result = decide_pick(&session, &cfg, &lookup, &display, Some((20, 1)), 0, 0);
        assert!(matches!(result, PickDecision::LockIn { .. }));
    }

    #[test]
    fn best_ban_target_returns_first_available() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into(), "Zed".into()];
        let (id, name) = best_ban_target(&cfg, &lookup, &display, &HashSet::new(), &HashSet::new()).unwrap();
        assert_eq!(id, 1);
        assert_eq!(name, "Ahri");
    }

    #[test]
    fn best_ban_target_skips_already_banned() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into(), "Zed".into()];
        let already_banned: HashSet<i64> = [1].into_iter().collect();
        let (id, _) = best_ban_target(&cfg, &lookup, &display, &already_banned, &HashSet::new()).unwrap();
        assert_eq!(id, 2);
    }

    #[test]
    fn best_ban_target_returns_none_when_all_banned() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into()];
        let already_banned: HashSet<i64> = [1].into_iter().collect();
        assert!(best_ban_target(&cfg, &lookup, &display, &already_banned, &HashSet::new()).is_none());
    }

    #[test]
    fn best_ban_target_returns_none_for_unknown_champion_name() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["DefinitelyNotAChampion".into()];
        assert!(best_ban_target(&cfg, &lookup, &display, &HashSet::new(), &HashSet::new()).is_none());
    }

    #[test]
    fn best_ban_target_skips_champion_hovered_by_teammate() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into(), "Zed".into()];
        // Ahri (id=1) is being hovered by a teammate for banning
        let teammate_hovers: HashSet<i64> = [1].into_iter().collect();
        let (id, name) = best_ban_target(&cfg, &lookup, &display, &HashSet::new(), &teammate_hovers).unwrap();
        assert_eq!(id, 2);
        assert_eq!(name, "Zed");
    }

    #[test]
    fn best_ban_target_returns_none_when_all_champions_covered_by_teammates() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.bans = vec!["Ahri".into(), "Zed".into()];
        // Both Ahri and Zed are already being hovered by teammates
        let teammate_hovers: HashSet<i64> = [1, 2].into_iter().collect();
        assert!(best_ban_target(&cfg, &lookup, &display, &HashSet::new(), &teammate_hovers).is_none());
    }

    // ── decide_ban: teammate hover integration ────────────────────────────────

    fn make_action_with_champ(id: i64, cell_id: i64, action_type: &str, in_progress: bool, completed: bool, champion_id: i64) -> Action {
        Action { id, actor_cell_id: cell_id, action_type: action_type.into(), is_in_progress: in_progress, completed, champion_id }
    }

    #[test]
    fn decide_ban_skips_to_next_when_teammate_hovering_first_choice() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // local player is cell 3; teammate (cell 1) is hovering Ahri (id=1) for ban
        let my_ban     = make_action(10, 3, "ban", true, false);
        let their_ban  = make_action_with_champ(11, 1, "ban", true, false, 1);
        let member     = make_member(3, "mid", 0);
        let session    = make_session(3, vec![vec![my_ban, their_ban]], vec![member], Bans::default(), "BAN_PICK");
        let result     = decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0);
        // Should hover Zed (id=2) instead of Ahri
        assert_eq!(result, BanDecision::Hover { action_id: 10, champion_id: 2, champion_name: "Zed".into() });
    }

    #[test]
    fn decide_ban_all_bans_exhausted_when_teammates_cover_every_choice() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // Both Ahri (id=1) and Zed (id=2) are being hovered by teammates
        let my_ban    = make_action(10, 3, "ban", true, false);
        let ban_ahri  = make_action_with_champ(11, 1, "ban", true, false, 1);
        let ban_zed   = make_action_with_champ(12, 2, "ban", true, false, 2);
        let member    = make_member(3, "mid", 0);
        let session   = make_session(3, vec![vec![my_ban, ban_ahri, ban_zed]], vec![member], Bans::default(), "BAN_PICK");
        let result    = decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0);
        assert_eq!(result, BanDecision::AllBansExhausted);
    }

    #[test]
    fn decide_ban_does_not_skip_own_hover() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // We (cell 3) are hovering Ahri — should not treat it as a teammate hover
        let my_ban = make_action_with_champ(10, 3, "ban", true, false, 1);
        let member = make_member(3, "mid", 0);
        let session = make_session(3, vec![vec![my_ban]], vec![member], Bans::default(), "BAN_PICK");
        // hovered_ban = Some(1): we've already hovered Ahri and have plenty of time
        let result = decide_ban(&session, &default_ban_config(), &lookup, &display, Some(1), 0);
        // Should wait for timer (20s remain > 5s threshold) — not skip Ahri
        assert!(matches!(result, BanDecision::WaitForTimer { .. }));
    }

    #[test]
    fn decide_ban_ignores_teammate_hover_with_zero_champion_id() {
        let (lookup, display) = build_champion_map(&make_summaries());
        // Teammate action has champion_id = 0 (not yet picked) — must not be treated as a hover
        let my_ban    = make_action(10, 3, "ban", true, false);
        let their_ban = make_action_with_champ(11, 1, "ban", true, false, 0);
        let member    = make_member(3, "mid", 0);
        let session   = make_session(3, vec![vec![my_ban, their_ban]], vec![member], Bans::default(), "BAN_PICK");
        let result    = decide_ban(&session, &default_ban_config(), &lookup, &display, None, 0);
        // Ahri should still be the first choice since the teammate hasn't hovered anything
        assert_eq!(result, BanDecision::Hover { action_id: 10, champion_id: 1, champion_name: "Ahri".into() });
    }

    // ── best_pick_target ──────────────────────────────────────────────────────

    #[test]
    fn best_pick_target_returns_first_available() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.preferences.mid = vec!["Ahri".into(), "Zed".into()];
        let (id, _) = best_pick_target(&cfg, "middle", &lookup, &display, &HashSet::new()).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn best_pick_target_skips_unavailable_champion() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.preferences.mid = vec!["Ahri".into(), "Zed".into()];
        let unavailable: HashSet<i64> = [1].into_iter().collect();
        let (id, _) = best_pick_target(&cfg, "mid", &lookup, &display, &unavailable).unwrap();
        assert_eq!(id, 2);
    }

    #[test]
    fn best_pick_target_returns_none_when_all_unavailable() {
        let (lookup, display) = build_champion_map(&make_summaries());
        let mut cfg = Config::default();
        cfg.preferences.mid = vec!["Ahri".into()];
        let unavailable: HashSet<i64> = [1].into_iter().collect();
        assert!(best_pick_target(&cfg, "mid", &lookup, &display, &unavailable).is_none());
    }
}
