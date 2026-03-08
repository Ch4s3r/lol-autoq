use anyhow::Result;
use inquire::{InquireError, Select, Text};

use crate::config::{Config, INSTANT};

const BACK: &str = "← Back";
const SAVE_EXIT: &str = "✓ Save & Exit";

fn format_threshold(secs: u64) -> String {
    if secs == INSTANT {
        "Instant".to_string()
    } else {
        format!("≤ {secs}s")
    }
}

/// Entry point for the configure subcommand.
pub fn run(config: &mut Config, champion_names: Option<Vec<String>>) -> Result<()> {
    println!();
    println!("  LoL Auto-Queue Configuration");
    println!("  Changes are saved when you choose \"Save & Exit\".");
    println!();

    loop {
        let options = vec![
            format!(
                "Champion Picks  {}",
                picks_summary(config)
            ),
            format!(
                "Bans            {}",
                if config.bans.is_empty() { "(none configured)".to_string() } else { config.bans.join(" → ") }
            ),
            format!(
                "Lock-in Timers  ban {} / pick {}",
                format_threshold(config.lock_in_ban_secs),
                format_threshold(config.lock_in_pick_secs)
            ),
            SAVE_EXIT.to_string(),
        ];

        let selection = match Select::new("What would you like to configure?", options).prompt() {
            Ok(s) => s,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Err(e) => return Err(e.into()),
        };

        if selection.starts_with("✓") {
            break;
        } else if selection.starts_with("Champion Picks") {
            edit_picks_menu(config, champion_names.as_deref())?;
        } else if selection.starts_with("Bans") {
            edit_position(config, "Bans", champion_names.as_deref())?;
        } else if selection.starts_with("Lock-in Timers") {
            edit_timers(config)?;
        }
    }

    config.save()?;
    println!();
    println!("  ✓ Configuration saved to config.toml");
    println!();
    Ok(())
}

// --------------------------------------------------------------------------
// Picks submenu
// --------------------------------------------------------------------------

fn picks_summary(config: &Config) -> String {
    let prefs = &config.preferences;
    let counts = [
        prefs.top.len(), prefs.jungle.len(), prefs.mid.len(),
        prefs.bot.len(), prefs.support.len(), prefs.fill.len(),
    ];
    let configured = counts.iter().filter(|&&c| c > 0).count();
    format!("{}/6 positions configured", configured)
}

fn edit_picks_menu(
    config: &mut Config,
    champion_names: Option<&[String]>,
) -> Result<()> {
    loop {
        let prefs = &config.preferences;
        let positions: &[(&str, &Vec<String>)] = &[
            ("Top",     &prefs.top),
            ("Jungle",  &prefs.jungle),
            ("Mid",     &prefs.mid),
            ("Bot",     &prefs.bot),
            ("Support", &prefs.support),
            ("Fill",    &prefs.fill),
        ];

        let mut options: Vec<String> = positions
            .iter()
            .map(|(name, champs)| {
                if champs.is_empty() {
                    format!("{name:<8}  (none configured)")
                } else {
                    format!("{name:<8}  {}", champs.join(" → "))
                }
            })
            .collect();
        options.push(BACK.to_string());

        let selection = match Select::new("Select a position to configure:", options).prompt() {
            Ok(s) => s,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Err(e) => return Err(e.into()),
        };

        if selection.starts_with("←") {
            break;
        }

        let position = selection.split_whitespace().next().unwrap_or("").to_string();
        edit_position(config, &position, champion_names)?;
    }
    Ok(())
}
/// Interactive editor for a single position's champion list.
fn edit_position(
    config: &mut Config,
    position: &str,
    champion_names: Option<&[String]>,
) -> Result<()> {
    loop {
        let list = champions_for_position_mut(config, position);

        println!();
        println!("  {position} — current pick order:");
        if list.is_empty() {
            println!("    (no champions configured)");
        } else {
            for (i, champ) in list.iter().enumerate() {
                println!("    {}. {champ}", i + 1);
            }
        }
        println!();

        let mut actions = vec!["Add champion(s)".to_string()];
        if !list.is_empty() {
            actions.push("Remove champion(s)".to_string());
            if list.len() > 1 {
                actions.push("Move champion up".to_string());
                actions.push("Move champion down".to_string());
            }
        }
        actions.push(BACK.to_string());

        let action = match Select::new("What would you like to do?", actions).prompt() {
            Ok(a) => a,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Err(e) => return Err(e.into()),
        };

        match action.as_str() {
            "Add champion(s)" => action_add(config, position, champion_names)?,
            "Remove champion(s)" => action_remove(config, position)?,
            "Move champion up" => action_move(config, position, -1)?,
            "Move champion down" => action_move(config, position, 1)?,
            _ => break, // Back
        }
    }
    Ok(())
}

fn action_add(
    config: &mut Config,
    position: &str,
    champion_names: Option<&[String]>,
) -> Result<()> {
    match champion_names {
        Some(all_names) => {
            // Loop so each pick appends in the order the user selects, not
            // alphabetically (MultiSelect returns display order, not pick order).
            const DONE: &str = "✓ Done adding";
            let mut added: Vec<String> = Vec::new();

            loop {
                // Exclude already-in-list champions AND ones picked this round.
                let current = champions_for_position_mut(config, position);
                let mut available: Vec<String> = all_names
                    .iter()
                    .filter(|n| {
                        !current.iter().any(|c| c.eq_ignore_ascii_case(n))
                            && !added.iter().any(|a| a.eq_ignore_ascii_case(n))
                    })
                    .cloned()
                    .collect();

                if available.is_empty() {
                    println!("  No more champions to add.");
                    break;
                }

                // Prepend the Done option so it stays at the top.
                available.insert(0, DONE.to_string());

                let pick = match Select::new(
                    &format!(
                        "Pick a champion to add (selected so far: {}):",
                        if added.is_empty() {
                            "none".to_string()
                        } else {
                            added.join(", ")
                        }
                    ),
                    available,
                )
                .prompt()
                {
                    Ok(v) => v,
                    Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                        break
                    }
                    Err(e) => return Err(e.into()),
                };

                if pick == DONE {
                    break;
                }
                added.push(pick);
            }

            if added.is_empty() {
                println!("  (nothing selected)");
                return Ok(());
            }

            let list = champions_for_position_mut(config, position);
            for name in &added {
                list.push(name.clone());
            }
            println!("  Added: {}.", added.join(", "));
        }
        None => {
            // Fall back to free-text when the client isn't running.
            let input = match Text::new("Champion name to add (comma-separated for multiple):").prompt() {
                Ok(n) => n,
                Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                    return Ok(())
                }
                Err(e) => return Err(e.into()),
            };

            let names: Vec<String> = input
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if names.is_empty() {
                println!("  (empty input — skipped)");
                return Ok(());
            }

            let list = champions_for_position_mut(config, position);
            let mut added = Vec::new();
            for name in names {
                if list.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
                    println!("  '{name}' is already in the list — skipped.");
                } else {
                    list.push(name.clone());
                    added.push(name);
                }
            }
            if !added.is_empty() {
                println!("  Added: {}.", added.join(", "));
            }
        }
    }
    Ok(())
}

fn action_remove(config: &mut Config, position: &str) -> Result<()> {
    const DONE: &str = "✓ Done removing";
    let mut removed: Vec<String> = Vec::new();

    loop {
        let current = champions_for_position_mut(config, position);
        // Show remaining champions (excluding ones already picked for removal).
        let mut available: Vec<String> = current
            .iter()
            .filter(|c| !removed.iter().any(|r| r.eq_ignore_ascii_case(c)))
            .cloned()
            .collect();

        if available.is_empty() {
            println!("  No more champions to remove.");
            break;
        }

        available.insert(0, DONE.to_string());

        let pick = match Select::new(
            &format!(
                "Pick a champion to remove (marked for removal: {}):",
                if removed.is_empty() {
                    "none".to_string()
                } else {
                    removed.join(", ")
                }
            ),
            available,
        )
        .prompt()
        {
            Ok(v) => v,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Err(e) => return Err(e.into()),
        };

        if pick == DONE {
            break;
        }
        removed.push(pick);
    }

    if removed.is_empty() {
        println!("  (nothing selected)");
        return Ok(());
    }

    let list = champions_for_position_mut(config, position);
    list.retain(|c| !removed.iter().any(|r| r.eq_ignore_ascii_case(c)));
    println!("  Removed: {}.", removed.join(", "));
    Ok(())
}

fn action_move(config: &mut Config, position: &str, delta: i32) -> Result<()> {
    let list = champions_for_position_mut(config, position);
    let options = list.clone();
    let direction = if delta < 0 { "Move up" } else { "Move down" };

    let chosen = match Select::new(&format!("{direction} — which champion?"), options).prompt() {
        Ok(c) => c,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let list = champions_for_position_mut(config, position);
    if let Some(idx) = list.iter().position(|c| c == &chosen) {
        let new_idx = (idx as i32 + delta).clamp(0, list.len() as i32 - 1) as usize;
        if new_idx != idx {
            list.swap(idx, new_idx);
            println!("  Moved '{chosen}' {}.", if delta < 0 { "up" } else { "down" });
        } else {
            println!("  '{chosen}' is already at that end of the list.");
        }
    }
    Ok(())
}

/// Interactive editor for lock-in timer thresholds.
fn edit_timers(config: &mut Config) -> Result<()> {
    println!();
    println!("  Lock-in Timers");
    println!("  'Instant' locks in as soon as the phase starts (champion is hovered first).");
    println!("  A number locks in when that many seconds or fewer remain.");
    println!("  0 = lock at the very last moment.");
    println!();
    println!("  Current:  ban {}  /  pick {}",
        format_threshold(config.lock_in_ban_secs),
        format_threshold(config.lock_in_pick_secs));
    println!();

    config.lock_in_ban_secs  = prompt_threshold("Ban",  config.lock_in_ban_secs)?;
    config.lock_in_pick_secs = prompt_threshold("Pick", config.lock_in_pick_secs)?;
    Ok(())
}

const TIMER_INSTANT: &str  = "Instant (lock as soon as hovered)";
const TIMER_CUSTOM: &str   = "Custom (enter seconds)";

fn prompt_threshold(label: &str, current: u64) -> Result<u64> {
    let current_str = format_threshold(current);
    let options = vec![
        TIMER_INSTANT.to_string(),
        "0  (last possible moment)".to_string(),
        "3s".to_string(),
        "5s".to_string(),
        "10s".to_string(),
        "15s".to_string(),
        "20s".to_string(),
        TIMER_CUSTOM.to_string(),
    ];

    let selection = match Select::new(
        &format!("{label} lock-in timing (current: {current_str}):"),
        options,
    ).prompt() {
        Ok(v) => v,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            println!("  Keeping current value ({current_str}).");
            return Ok(current);
        }
        Err(e) => return Err(e.into()),
    };

    if selection == TIMER_INSTANT {
        println!("  {label} lock-in set to Instant.");
        return Ok(INSTANT);
    }

    if selection.starts_with('0') {
        println!("  {label} lock-in set to last possible moment (0s).");
        return Ok(0);
    }

    // Fixed shortcut values like "5s"
    if selection != TIMER_CUSTOM
        && let Ok(secs) = selection.trim_end_matches('s').parse::<u64>() {
            println!("  {label} lock-in set to ≤ {secs}s.");
            return Ok(secs);
        }

    // Custom free-text
    let input = match Text::new("Enter seconds (0 = last moment, higher = earlier lock-in):")
        .with_default(&if current == INSTANT { "5".to_string() } else { current.to_string() })
        .prompt()
    {
        Ok(v) => v,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            println!("  Keeping current value ({current_str}).");
            return Ok(current);
        }
        Err(e) => return Err(e.into()),
    };

    match input.trim().parse::<u64>() {
        Ok(secs) => {
            println!("  {label} lock-in set to ≤ {secs}s.");
            Ok(secs)
        }
        Err(_) => {
            println!("  Invalid number — keeping current value ({current_str}).");
            Ok(current)
        }
    }
}

fn champions_for_position_mut<'a>(config: &'a mut Config, position: &str) -> &'a mut Vec<String> {
    match position {
        "Top" => &mut config.preferences.top,
        "Jungle" => &mut config.preferences.jungle,
        "Mid" => &mut config.preferences.mid,
        "Bot" => &mut config.preferences.bot,
        "Support" => &mut config.preferences.support,
        "Bans" => &mut config.bans,
        _ => &mut config.preferences.fill,
    }
}
