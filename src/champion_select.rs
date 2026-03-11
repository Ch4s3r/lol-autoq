use std::collections::{HashMap, HashSet};

use anyhow::Result;
use tracing::{info, trace, warn};

use crate::config::{Config, INSTANT};
use crate::lcu::{Action, ChampSelectSession, ChampionSummary, LcuClient};

/// Returns:
/// - `lookup`  : lowercase name/alias → champion id  (for resolving config preferences)
/// - `display` : champion id → proper display name    (for human-readable log messages)
pub fn build_champion_map(
    summaries: &[ChampionSummary],
) -> (HashMap<String, i64>, HashMap<i64, String>) {
    let valid: Vec<_> = summaries.iter().filter(|c| c.id > 0).collect();
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

/// Ban phase handler.
/// - Hovers the highest-priority available ban immediately.
/// - Locks the ban in when the timer reaches <= `lock_in_ban_secs`.
///
/// Returns `true` when the ban was completed, `false` when still waiting.
pub async fn handle_ban_phase(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_ban: &mut Option<i64>,
) -> Result<bool> {
    let action = match find_active_ban_action(session) {
        Some(a) => a,
        None => return Ok(false),
    };

    // Only act during the actual ban phase. During "PLANNING" the ban action
    // is already marked is_in_progress but it's too early to hover or lock.
    let phase = session.timer.phase.as_str();
    if phase == "PLANNING" || phase.is_empty() {
        trace!(phase, "skipping ban — not in ban phase yet");
        return Ok(false);
    }

    if config.bans.is_empty() {
        warn!("no ban preferences configured — add some via `lol-autoq configure`");
        return Ok(false);
    }

    // Champions already banned by either team — we cannot ban these again.
    let already_banned: HashSet<i64> = session
        .bans
        .my_team_bans
        .iter()
        .chain(session.bans.their_team_bans.iter())
        .copied()
        .collect();

    let (chosen_id, chosen_name) = match best_ban_target(config, champion_map, display_names, &already_banned) {
        Some(pair) => pair,
        None => {
            warn!("All preferred bans are already banned — add more options");
            return Ok(false);
        }
    };

    // Hover immediately if we haven't hovered this champion yet.
    if *hovered_ban != Some(chosen_id) {
        info!(champion = %chosen_name, "Hovering ban...");
        client.hover_champion(action.id, chosen_id).await?;
        *hovered_ban = Some(chosen_id);
        // Let the LCU process the hover before we try to lock.
        return Ok(false);
    }

    // Lock in when the timer drops to the threshold (INSTANT skips the check).
    let remaining_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
    if config.lock_in_ban_secs != INSTANT {
        let threshold = config.lock_in_ban_secs as f64;
        if remaining_secs > threshold {
            trace!(
                remaining = format!("{remaining_secs:.1}s"),
                threshold = format!("{threshold:.0}s"),
                champion = %chosen_name,
                "waiting to lock ban"
            );
            return Ok(false);
        }
    }

    info!(
        champion = %chosen_name,
        remaining = format!("{remaining_secs:.1}s"),
        ban_order = %config.bans.join(" -> "),
        "Locking in ban!"
    );
    client.lock_champion(action.id, chosen_id).await?;
    info!(champion = %chosen_name, "Ban complete!");
    Ok(true)
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
    session
        .my_team
        .iter()
        .filter(|m| m.champion_id != 0)
        .map(|m| m.champion_id)
        .chain(session.bans.my_team_bans.iter().copied())
        .chain(session.bans.their_team_bans.iter().copied())
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

/// Core champion-select handler.
/// - Hovers the highest-priority available champion immediately.
/// - Re-evaluates if the hovered champion gets banned and switches to the next.
/// - Locks in when the timer reaches <= `lock_in_pick_secs`.
///
/// Returns `true` when the champion was locked in, `false` when still waiting.
pub async fn handle_champion_select(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_pick: &mut Option<(i64, i64)>,
) -> Result<bool> {
    // Use the any-state action for hovering; need in-progress for lock-in.
    let action = match find_pick_action(session) {
        Some(a) => a,
        None => return Ok(false),
    };

    let raw_position = local_assigned_position(session);
    let position_label = friendly_position(raw_position);

    let prefs = config.champions_for_position(raw_position);
    if prefs.is_empty() {
        warn!(
            position = %position_label,
            "no champion preferences configured — please edit config.toml"
        );
        return Ok(false);
    }

    let unavailable = unavailable_champion_ids(session);
    trace!(ids = ?unavailable, "unavailable champions");

    let (chosen_id, chosen_name) = match best_pick_target(config, raw_position, champion_map, display_names, &unavailable) {
        Some(pair) => pair,
        None => {
            warn!(
                position = %position_label,
                "All preferred champions are banned/picked — add more options to config.toml"
            );
            return Ok(false);
        }
    };

    // Hover immediately, even before it's our turn.
    // Re-hover if the action ID or champion changed (e.g. intent → pick phase transition).
    if *hovered_pick != Some((action.id, chosen_id)) {
        info!(
            position = %position_label,
            champion = %chosen_name,
            pick_order = %prefs.join(" -> "),
            "Hovering champion..."
        );
        client.hover_champion(action.id, chosen_id).await?;
        *hovered_pick = Some((action.id, chosen_id));
    }

    // Only lock in when it's actually our turn in the pick phase
    // (not during the simultaneous intent phase at the start).
    if !action.is_in_progress || !all_bans_completed(session) {
        return Ok(false);
    }

    // Lock in when the timer drops to the threshold (INSTANT skips the check).
    let remaining_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
    if config.lock_in_pick_secs != INSTANT {
        let threshold = config.lock_in_pick_secs as f64;
        if remaining_secs > threshold {
            trace!(
                remaining = format!("{remaining_secs:.1}s"),
                threshold = format!("{threshold:.0}s"),
                champion = %chosen_name,
                "waiting to lock pick"
            );
            return Ok(false);
        }
    }

    // Re-check availability right before locking (champion could have been
    // banned between the hover and now).
    let unavailable = unavailable_champion_ids(session);
    if unavailable.contains(&chosen_id) {
        warn!(champion = %chosen_name, "champion was banned since hovering — switching");
        *hovered_pick = None;
        return Ok(false);
    }

    info!(
        position = %position_label,
        champion = %chosen_name,
        remaining = format!("{remaining_secs:.1}s"),
        "Locking in champion!"
    );
    client.lock_champion(action.id, chosen_id).await?;
    info!(champion = %chosen_name, "Lock-in complete!");
    Ok(true)
}
