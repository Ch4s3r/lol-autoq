pub mod components;
pub mod dashboard;
pub mod settings;
pub mod styles;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use tokio::time::sleep;

use crate::app_state::{ActivityKind, AppState, ConnectionState, GamePhase};
use crate::champion_select::{build_champion_map, handle_ban_phase, handle_champion_select};
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

fn poll_interval(phase: &str) -> Duration {
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
    let mut ban_jitter: u64 = 0;
    let mut hover_jitter: u64 = 0;
    let mut pick_jitter: u64 = 0;
    let mut queue_jitter: u64 = 0;

    loop {
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

            let cfg = state.config.read();
            ban_jitter   = roll_jitter(cfg.timer_jitter_secs);
            hover_jitter = roll_jitter(cfg.timer_jitter_secs);
            pick_jitter  = roll_jitter(cfg.timer_jitter_secs);
            queue_jitter = roll_jitter(cfg.timer_jitter_secs);
        }

        let cfg: Config = state.config.read().clone();

        match phase.as_str() {
            "ReadyCheck" => {
                if !ready_check_accepted {
                    let effective_delay = if cfg.accept_queue_delay_secs == INSTANT {
                        queue_jitter
                    } else {
                        cfg.accept_queue_delay_secs.saturating_add(queue_jitter)
                    };
                    let seen_at = ready_check_seen_at.get_or_insert_with(Instant::now);
                    if seen_at.elapsed() >= Duration::from_secs(effective_delay)
                        && client.accept_ready_check().await.is_ok() {
                            state.push_activity("Queue accepted!", ActivityKind::Success);
                            ready_check_accepted = true;
                        }
                }
            }

            "ChampSelect" => {
                if !ban_completed || !champ_locked {
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
                            hover_jitter,
                            pick_jitter,
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
                            ban_jitter,
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
                }
            }

            _ => {}
        }

        sleep(poll_interval(&phase)).await;
    }
}

fn roll_jitter(max_secs: u64) -> u64 {
    if max_secs == 0 {
        return 0;
    }
    rand::random_range(0..=max_secs)
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
