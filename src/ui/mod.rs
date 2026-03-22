pub mod components;
pub mod dashboard;
pub mod settings;
pub mod styles;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use tokio::time::sleep;

use crate::app_state::{ActivityKind, AppState, BanStatus, ChampSelectStatus, ConnectionState, GamePhase, HoverStatus, PickStatus};
use crate::champion_select::{build_champion_map, decide_ban, decide_pick, handle_ban_phase, handle_champion_select, BanDecision, PickDecision};
use crate::config::{Config, INSTANT};
use crate::lcu::{LcuClient, LockfileData};

use dashboard::Dashboard;
use settings::Settings;

// ── Navigation tab ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Dashboard,
    Settings,
}

// ── Root app component ────────────────────────────────────────────────────

#[component]
pub fn App() -> Element {
    let state = use_context_provider(AppState::new);
    let mut active_tab = use_signal(|| Tab::Dashboard);

    // Spawn the background LCU poll loop
    use_coroutine(move |_rx: UnboundedReceiver<()>| async move {
        bg_poll_loop(state).await;
    });

    rsx! {
        div { class: "app",
            // Page content
            if *active_tab.read() == Tab::Dashboard {
                Dashboard {}
            } else {
                Settings {}
            }

            // Bottom navigation
            div { class: "bottom-nav",
                button {
                    class: if *active_tab.read() == Tab::Dashboard { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| active_tab.set(Tab::Dashboard),
                    span { class: "nav-btn-icon", i { class: "fa-solid fa-table-cells-large" } }
                    span { "Dashboard" }
                }
                button {
                    class: if *active_tab.read() == Tab::Settings { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| active_tab.set(Tab::Settings),
                    span { class: "nav-btn-icon", i { class: "fa-solid fa-gear" } }
                    span { "Settings" }
                }
            }
        }
    }
}

// ── Background poll loop ──────────────────────────────────────────────────

const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const POLL_ACTIVE: Duration = Duration::from_millis(100);
const POLL_POSTGAME: Duration = Duration::from_secs(2);
const POLL_INGAME: Duration = Duration::from_secs(30);

pub(crate) fn poll_interval(phase: &str) -> Duration {
    match phase {
        "InProgress" => POLL_INGAME,
        "WaitingForStats" | "PreEndOfGame" | "EndOfGame" => POLL_POSTGAME,
        _ => POLL_ACTIVE,
    }
}

async fn bg_poll_loop(mut state: AppState) {
    loop {
        // 1. Find lockfile
        let lockfile = {
            let path = state.config.read().lockfile_path.clone();
            loop {
                match LockfileData::find(path.as_deref()) {
                    Ok(l) => break l,
                    Err(_) => {
                        state.connection.set(ConnectionState::Disconnected);
                        sleep(RECONNECT_INTERVAL).await;
                    }
                }
            }
        };

        // 2. Create LCU client
        let client = match LcuClient::new(&lockfile) {
            Ok(c) => {
                state.connection.set(ConnectionState::Connected { port: lockfile.port });
                c
            }
            Err(_) => {
                state.connection.set(ConnectionState::Disconnected);
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        // 3. Try to fetch DDragon version (best-effort)
        if let Ok(ver) = fetch_ddragon_version().await {
            state.ddragon_version.set(ver);
        }

        // 4. Load champion data (retry up to 10× with backoff)
        let (champion_map, display_names, summaries) =
            match load_champion_data_with_retry(&client).await {
                Ok(data) => data,
                Err(_) => {
                    state.connection.set(ConnectionState::Disconnected);
                    sleep(RECONNECT_INTERVAL).await;
                    continue;
                }
            };

        state.champion_summaries.set(summaries);
        state.push_activity(
            format!("Connected — {} champions loaded", display_names.len()),
            ActivityKind::Success,
        );

        // 5. Inner poll loop
        if inner_poll_loop(&client, state, &champion_map, &display_names).await.is_err() {
            state.push_activity("Lost connection to LCU", ActivityKind::Warning);
            state.connection.set(ConnectionState::Disconnected);
            state.phase.set(GamePhase::None);
            state.hovered_champion.set(None);
            sleep(RECONNECT_INTERVAL).await;
        }
    }
}

async fn inner_poll_loop(
    client: &LcuClient,
    mut state: AppState,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
) -> anyhow::Result<()> {
    let mut last_phase = String::new();
    let mut ready_check_accepted = false;
    let mut ready_check_seen_at: Option<Instant> = None;
    let mut ban_completed = false;
    let mut champ_locked = false;
    let mut hovered_ban: Option<i64> = None;
    let mut hovered_pick: Option<(i64, i64)> = None;

    loop {
        // Drain tracing events into the UI activity log (keeps file and UI in sync).
        state.drain_log_buffer();

        let phase = client.get_gameflow_phase().await?;

        if phase != last_phase {
            state.phase.set(GamePhase::from_lcu(&phase));
            state.push_activity(
                format!("→ {}", GamePhase::from_lcu(&phase).label()),
                ActivityKind::Info,
            );

            last_phase = phase.clone();
            ready_check_accepted = false;
            ready_check_seen_at = None;
            ban_completed = false;
            champ_locked = false;
            hovered_ban = None;
            hovered_pick = None;
            state.hovered_champion.set(None);
            state.champ_select_status.set(None);

        }

        let cfg: Config = state.config.read().clone();

        match phase.as_str() {
            "ReadyCheck" => {
                if !ready_check_accepted {
                    let delay = cfg.accept_queue_delay_secs;
                    let seen_at = ready_check_seen_at.get_or_insert_with(Instant::now);
                    if (delay == INSTANT || seen_at.elapsed() >= Duration::from_secs(delay))
                        && client.accept_ready_check().await.is_ok() {
                            state.push_activity("Queue accepted!", ActivityKind::Success);
                            ready_check_accepted = true;
                        }
                }
            }

            "ChampSelect" => {
                let session = match client.get_champ_select_session().await {
                    Ok(s) => s,
                    Err(_) => {
                        sleep(POLL_ACTIVE).await;
                        continue;
                    }
                };

                // Pick handling
                if !champ_locked {
                    let prev_hover = hovered_pick;
                    if let Ok(locked) = handle_champion_select(
                        client,
                        &session,
                        &cfg,
                        champion_map,
                        display_names,
                        &mut hovered_pick,
                    )
                    .await {
                        if hovered_pick != prev_hover
                            && let Some((_, id)) = hovered_pick {
                                let name = display_names.get(&id).cloned().unwrap_or_default();
                                state.hovered_champion.set(Some(name.clone()));
                                state.push_activity(format!("Hovering pick: {name}"), ActivityKind::Info);
                            }
                        if locked {
                            champ_locked = true;
                            if let Some((_, id)) = hovered_pick {
                                let name = display_names.get(&id).cloned().unwrap_or_default();
                                state.push_activity(format!("Locked in: {name}"), ActivityKind::Success);
                            }
                        }
                    }
                }

                // Ban handling
                if !ban_completed {
                    let prev_ban = hovered_ban;
                    match handle_ban_phase(
                        client,
                        &session,
                        &cfg,
                        champion_map,
                        display_names,
                        &mut hovered_ban,
                    )
                    .await
                    {
                        Ok(true) => {
                            if let Some(id) = hovered_ban {
                                let name = display_names.get(&id).cloned().unwrap_or_default();
                                state.push_activity(format!("Banned: {name}"), ActivityKind::Success);
                            }
                            ban_completed = true;
                        }
                        Ok(false) => {
                            if hovered_ban != prev_ban
                                && let Some(id) = hovered_ban {
                                    let name = display_names.get(&id).cloned().unwrap_or_default();
                                    state.push_activity(format!("Hovering ban: {name}"), ActivityKind::Info);
                                }
                        }
                        Err(_) => {}
                    }
                }

                // Update live champ-select status signal (always, to keep countdown live)
                let time_left_secs = session.timer.adjusted_time_left_ms as f64 / 1000.0;
                let sub_phase      = session.timer.phase.clone();
                let ban_status   = derive_ban_status(&session, &cfg, champion_map, display_names, hovered_ban, ban_completed);
                let hover_status = derive_hover_status(&session, &cfg, champion_map, display_names, hovered_pick, champ_locked);
                let pick_status  = derive_pick_status(&session, &cfg, champion_map, display_names, hovered_pick, champ_locked);
                state.champ_select_status.set(Some(ChampSelectStatus {
                    time_left_secs,
                    sub_phase,
                    hover: hover_status,
                    ban:   ban_status,
                    pick:  pick_status,
                }));
            }

            _ => {}
        }

        sleep(poll_interval(&phase)).await;
    }
}

fn derive_ban_status(
    session: &crate::lcu::ChampSelectSession,
    config: &crate::config::Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_ban: Option<i64>,
    ban_completed: bool,
) -> BanStatus {
    if ban_completed {
        let name = hovered_ban.and_then(|id| display_names.get(&id)).cloned()
            .unwrap_or_else(|| "Unknown".to_string());
        return BanStatus::Banned { champion_name: name };
    }
    match decide_ban(session, config, champion_map, display_names, hovered_ban) {
        BanDecision::Idle                                   => BanStatus::Idle,
        BanDecision::NoBansConfigured                       => BanStatus::NoBansConfigured,
        BanDecision::AllBansExhausted                       => BanStatus::AllBansExhausted,
        BanDecision::WaitForTimer { champion_name, .. }     => BanStatus::WaitingToLock { champion_name },
        BanDecision::Hover  { champion_name, .. }           => BanStatus::Hovering { champion_name },
        BanDecision::LockIn { champion_name, .. }           => BanStatus::Hovering { champion_name },
    }
}

fn derive_hover_status(
    session: &crate::lcu::ChampSelectSession,
    config: &crate::config::Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_pick: Option<(i64, i64)>,
    champ_locked: bool,
) -> HoverStatus {
    if champ_locked {
        let name = hovered_pick.and_then(|(_, id)| display_names.get(&id)).cloned()
            .unwrap_or_else(|| "Unknown".to_string());
        return HoverStatus::LockedIn { champion_name: name };
    }
    match decide_pick(session, config, champion_map, display_names, hovered_pick) {
        PickDecision::Idle                                       => HoverStatus::Idle,
        PickDecision::NoPrefsConfigured { position }             => HoverStatus::NoPrefsConfigured { position },
        PickDecision::AllPicksExhausted { position }             => HoverStatus::AllPicksExhausted { position },
        PickDecision::WaitForHoverTimer { champion_name, .. }    => HoverStatus::WaitingToHover { champion_name },
        PickDecision::Hover { champion_name, .. }                => HoverStatus::Hovering { champion_name },
        PickDecision::WaitForLockIn                              => HoverStatus::Hovering {
            champion_name: hovered_pick
                .and_then(|(_, id)| display_names.get(&id))
                .cloned()
                .unwrap_or_else(|| "Waiting…".to_string()),
        },
        PickDecision::StaleHover { .. }                          => HoverStatus::Idle,
        PickDecision::LockIn { champion_name, .. }               => HoverStatus::Hovering { champion_name },
    }
}

fn derive_pick_status(
    session: &crate::lcu::ChampSelectSession,
    config: &crate::config::Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
    hovered_pick: Option<(i64, i64)>,
    champ_locked: bool,
) -> PickStatus {
    if champ_locked {
        let name = hovered_pick.and_then(|(_, id)| display_names.get(&id)).cloned()
            .unwrap_or_else(|| "Unknown".to_string());
        return PickStatus::LockedIn { champion_name: name };
    }
    match decide_pick(session, config, champion_map, display_names, hovered_pick) {
        PickDecision::Idle                                       => PickStatus::Idle,
        PickDecision::NoPrefsConfigured { .. }                   => PickStatus::Idle,
        PickDecision::AllPicksExhausted { .. }                   => PickStatus::Idle,
        PickDecision::WaitForHoverTimer { champion_name, .. }    => PickStatus::WaitingToLock { champion_name },
        PickDecision::Hover { champion_name, .. }                => PickStatus::WaitingToLock { champion_name },
        PickDecision::WaitForLockIn                              => PickStatus::WaitingToLock {
            champion_name: hovered_pick
                .and_then(|(_, id)| display_names.get(&id))
                .cloned()
                .unwrap_or_else(|| "Waiting…".to_string()),
        },
        PickDecision::StaleHover { .. }                          => PickStatus::Idle,
        PickDecision::LockIn { champion_name, .. }               => PickStatus::WaitingToLock { champion_name },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── poll_interval ─────────────────────────────────────────────────────────

    #[test]
    fn poll_interval_ingame_is_30s() {
        assert_eq!(poll_interval("InProgress"), Duration::from_secs(30));
    }

    #[test]
    fn poll_interval_postgame_phases_are_2s() {
        assert_eq!(poll_interval("WaitingForStats"), Duration::from_secs(2));
        assert_eq!(poll_interval("PreEndOfGame"),    Duration::from_secs(2));
        assert_eq!(poll_interval("EndOfGame"),       Duration::from_secs(2));
    }

    #[test]
    fn poll_interval_active_phases_are_100ms() {
        for phase in &["ReadyCheck", "ChampSelect", "Lobby", "None", "Matchmaking", ""] {
            assert_eq!(poll_interval(phase), Duration::from_millis(100), "failed for phase {phase:?}");
        }
    }
}

async fn load_champion_data_with_retry(
    client: &LcuClient,
) -> anyhow::Result<(HashMap<String, i64>, HashMap<i64, String>, Vec<crate::lcu::ChampionSummary>)>
{
    const MAX: u32 = 10;
    let mut last_err = None;
    for attempt in 0..MAX {
        match client.get_champion_summary().await {
            Ok(summaries) => {
                let (map, display) = build_champion_map(&summaries);
                return Ok((map, display, summaries));
            }
            Err(e) => {
                let delay = Duration::from_secs(2) * 2u32.pow(attempt.min(4));
                sleep(delay).await;
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap())
}

async fn fetch_ddragon_version() -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let versions: Vec<String> = client
        .get("https://ddragon.leagueoflegends.com/api/versions.json")
        .send()
        .await?
        .json()
        .await?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty versions list"))
}
