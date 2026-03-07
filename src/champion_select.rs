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

/// Core champion-select handler. Hovers then optionally locks in our pick.
pub async fn handle_champion_select(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    auto_lock: bool,
) -> Result<()> {
    let action = match find_active_pick_action(session) {
        Some(a) => a,
        None => return Ok(()), // nothing to do yet
    };

    let raw_position = local_assigned_position(session);
    let position_label = friendly_position(raw_position);

    let prefs = config.champions_for_position(raw_position);
    if prefs.is_empty() {
        warn!(
            position = %position_label,
            "no champion preferences configured — please edit config.toml"
        );
        return Ok(());
    }

    info!(
        position = %position_label,
        pick_order = %prefs.join(" -> "),
        "champion select"
    );

    let unavailable = unavailable_champion_ids(session);
    trace!(ids = ?unavailable, "unavailable champions");

    let (chosen_id, chosen_name) = prefs
        .iter()
        .find_map(|pref_name| {
            let key = pref_name.to_ascii_lowercase();
            match champion_map.get(&key) {
                None => {
                    warn!(champion = %pref_name, "not found in game data — check spelling in config.toml");
                    None
                }
                Some(&id) if unavailable.contains(&id) => {
                    info!(champion = %pref_name, "banned or already picked — skipping");
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

    // Only hover if we are not already on this champion.
    if action.champion_id != chosen_id {
        info!(champion = %chosen_name, "Hovering...");
        client.hover_champion(action.id, chosen_id).await?;
    }

    if auto_lock {
        info!(champion = %chosen_name, "Locked in!");
        client.lock_champion(action.id).await?;
    }

    Ok(())
}
