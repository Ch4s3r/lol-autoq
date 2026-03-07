use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use tracing::{info, trace, warn};

use crate::config::Config;
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

/// Ban phase handler.
/// - Hovers the highest-priority available ban immediately.
/// - Locks the ban in when the timer reaches <= BAN_AT_MS.
///
/// Returns `true` when the ban was completed, `false` when still waiting.
pub async fn handle_ban_phase(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
) -> Result<bool> {
    const BAN_AT_MS: i64 = 3_000;

    let action = match find_active_ban_action(session) {
        Some(a) => a,
        None => return Ok(false),
    };

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

    let (chosen_id, chosen_name) = config
        .bans
        .iter()
        .find_map(|pref_name| {
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
        .ok_or_else(|| anyhow!("All preferred bans are already banned — add more options"))?;

    let time_left_ms = session.timer.adjusted_time_left_ms;

    if action.champion_id != chosen_id {
        info!(
            champion = %chosen_name,
            ban_order = %config.bans.join(" -> "),
            "Hovering ban..."
        );
        client.hover_champion(action.id, chosen_id).await?;
    }

    if time_left_ms <= BAN_AT_MS {
        info!(champion = %chosen_name, time_left_ms, "Banning!");
        client.lock_champion(action.id, chosen_id).await?;
        Ok(true)
    } else {
        trace!(time_left_ms, ban_at_ms = BAN_AT_MS, "waiting to ban");
        Ok(false)
    }
}

/// Return the single pick action that belongs to `local_player_cell_id`
/// and is currently in-progress and not yet completed.
pub fn find_active_pick_action(session: &ChampSelectSession) -> Option<&Action> {
    session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == session.local_player_cell_id
            && a.action_type == "pick"
            && a.is_in_progress
            && !a.completed
    })
}

/// Collect all champion ids that are currently banned or picked
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

/// Core champion-select handler.
/// - Hovers immediately so the team can see the intent.
/// - Locks in only when `time_left_ms` drops to <= `LOCK_AT_MS`.
///
/// Returns `true` when the champion was locked in, `false` when still waiting.
pub async fn handle_champion_select(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
) -> Result<bool> {
    const LOCK_AT_MS: i64 = 5_000;
    let action = match find_active_pick_action(session) {
        Some(a) => a,
        None => return Ok(false), // nothing to do yet
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

    let (chosen_id, chosen_name) = prefs
        .iter()
        .find_map(|pref_name| {
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
        .ok_or_else(|| {
            anyhow!(
                "All preferred champions are banned/picked for {} — add more options to config.toml",
                position_label
            )
        })?;

    trace!(champion_id = chosen_id, champion = %chosen_name, "champion selected");

    let time_left_ms = session.timer.adjusted_time_left_ms;

    // Hover once — log position/pick order only on the first hover so it doesn't
    // spam every poll cycle while waiting to lock in.
    if action.champion_id != chosen_id {
        info!(
            position = %position_label,
            pick_order = %prefs.join(" -> "),
            "champion select"
        );
        info!(champion = %chosen_name, time_left_ms, "Hovering...");
        client.hover_champion(action.id, chosen_id).await?;
    }

    // Lock in only once the timer reaches 5 seconds or less.
    if time_left_ms <= LOCK_AT_MS {
        info!(champion = %chosen_name, time_left_ms, "Locking in!");
        client.lock_champion(action.id, chosen_id).await?;
        Ok(true)
    } else {
        trace!(time_left_ms, total_ms = session.timer.total_time_ms, lock_at_ms = LOCK_AT_MS, "waiting to lock in");
        Ok(false)
    }
}
