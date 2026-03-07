mod champion_select;
mod cli;
mod config;
mod configure;
mod lcu;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use clap::Parser as _;
use tokio::time::sleep;
use tracing::{error, info, trace, warn};

use champion_select::{build_champion_map, handle_champion_select};
use cli::{Cli, Command};
use config::Config;
use lcu::{LcuClient, LockfileData};

/// How long to wait between polls when the client is not running.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
/// Active phases: ready check and champ select need fast reaction.
const POLL_FAST: Duration = Duration::from_millis(500);
/// Idle phases: lobby / post-game, nothing time-sensitive.
const POLL_IDLE: Duration = Duration::from_secs(3);
/// In-game: nothing to do until the game ends, check infrequently.
const POLL_INGAME: Duration = Duration::from_secs(30);

/// Return the appropriate poll interval for a given gameflow phase.
fn poll_interval(phase: &str) -> Duration {
    match phase {
        // Time-critical: must react within a second.
        "Matchmaking" | "ReadyCheck" | "ChampSelect" | "GameStart" => POLL_FAST,
        // Active game: nothing actionable, wake up rarely.
        "InProgress" => POLL_INGAME,
        // Lobby, post-game, unknown: moderate cadence.
        _ => POLL_IDLE,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Configure => {
            let mut config = Config::load_or_create()?;
            configure::run(&mut config)?;
        }

        Command::Start => {
            tracing_subscriber::fmt()
                .with_target(false)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .init();

            println!();
            println!("  =================================");
            println!("   LoL Auto-Queue  v{}", env!("CARGO_PKG_VERSION"));
            println!("   Auto-accept queues & pick champs");
            println!("  =================================");
            println!();

            let config = Config::load_or_create()?;

            info!("Champion preferences (edit config.toml or run `lol-autoq configure` to change):");
            info!("  Top:     {}", config.preferences.top.join(" -> "));
            info!("  Jungle:  {}", config.preferences.jungle.join(" -> "));
            info!("  Mid:     {}", config.preferences.mid.join(" -> "));
            info!("  Bot:     {}", config.preferences.bot.join(" -> "));
            info!("  Support: {}", config.preferences.support.join(" -> "));
            info!("  Fill:    {}", config.preferences.fill.join(" -> "));
            info!("");
            info!("Waiting for the League of Legends client to start...");

            run_loop(&config).await?;
        }
    }

    Ok(())
}

async fn run_loop(config: &Config) -> Result<()> {
    let mut client_connected = false;

    loop {
        let lockfile = match LockfileData::find(config.lockfile_path.as_deref()) {
            Ok(l) => l,
            Err(_) => {
                if client_connected {
                    // We just lost the client.
                    warn!("League client closed — waiting for it to restart...");
                    client_connected = false;
                }
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        let client = match LcuClient::new(&lockfile) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to League client: {}", e);
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        info!(port = lockfile.port, "League client found — connected");
        client_connected = true;

        // Load champion data once per connection.
        let (champion_map, display_names) = match load_champion_map(&client).await {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "Failed to load champion data");
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };
        info!(count = display_names.len(), "Champion data loaded — ready!");
        info!("");

        // Inner poll loop – runs until an error indicates the client closed.
        if let Err(e) = poll_loop(&client, config, &champion_map, &display_names).await {
            warn!("Lost connection to League client ({})", e);
            client_connected = false;
            sleep(RECONNECT_INTERVAL).await;
        }
    }
}

/// Translate a raw LCU gameflow phase string into a user-friendly label.
fn phase_label(phase: &str) -> &str {
    match phase {
        "None" | "Lobby" => "In lobby",
        "Matchmaking" => "Searching for a match...",
        "ReadyCheck" => "Ready check!",
        "ChampSelect" => "Champion select",
        "GameStart" => "Game is starting...",
        "InProgress" => "Game in progress",
        "WaitingForStats" | "PreEndOfGame" | "EndOfGame" => "Game over — back to lobby soon",
        other => other,
    }
}

/// Steady-state poll loop. Returns an error when the LCU is unreachable.
async fn poll_loop(
    client: &LcuClient,
    config: &Config,
    champion_map: &HashMap<String, i64>,
    display_names: &HashMap<i64, String>,
) -> Result<()> {
    let mut last_phase = String::new();
    let mut ready_check_accepted = false;
    let mut champ_locked = false;

    loop {
        let phase = client.get_gameflow_phase().await?;

        if phase != last_phase {
            info!(phase = phase_label(&phase), "game state changed");
            last_phase = phase.clone();
            // Reset per-phase state when we transition.
            ready_check_accepted = false;
            champ_locked = false;
        }

        match phase.as_str() {
            "ReadyCheck" => {
                if !ready_check_accepted {
                    trace!("attempting to accept ready check");
                    match client.accept_ready_check().await {
                        Ok(()) => {
                            info!("Queue accepted! Getting into champ select...");
                            ready_check_accepted = true;
                        }
                        Err(e) => warn!(error = %e, "Could not accept queue"),
                    }
                }
            }

            "ChampSelect" => {
                if !champ_locked {
                    let session = match client.get_champ_select_session().await {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(error = %e, "Could not read champ select session");
                            sleep(POLL_FAST).await;
                            continue;
                        }
                    };

                    match handle_champion_select(
                        client,
                        &session,
                        config,
                        champion_map,
                        display_names,
                        /* auto_lock = */ true,
                    )
                    .await
                    {
                        Ok(()) => {
                            // Mark as locked only if a pick action was in progress.
                            if champion_select::find_active_pick_action(&session).is_some() {
                                champ_locked = true;
                            }
                        }
                        Err(e) => warn!(error = %e, "Champion select error"),
                    }
                }
            }

            // Nothing actionable in other phases.
            _ => {}
        }

        sleep(poll_interval(&phase)).await;
    }
}

async fn load_champion_map(
    client: &LcuClient,
) -> Result<(HashMap<String, i64>, HashMap<i64, String>)> {
    let summaries = client.get_champion_summary().await?;
    Ok(build_champion_map(&summaries))
}

