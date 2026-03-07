use anyhow::Result;
use inquire::{InquireError, Select, Text};

use crate::config::{Config, LanePreferences};

const BACK: &str = "← Back";
const SAVE_EXIT: &str = "✓ Save & Exit";

/// Entry point for the configure subcommand.
pub fn run(config: &mut Config) -> Result<()> {
    println!();
    println!("  Champion Preference Configuration");
    println!("  Pick a position to edit, then manage your champion priority list.");
    println!("  Changes are saved when you choose \"Save & Exit\".");
    println!();

    loop {
        let options = position_menu_options(&config.preferences);
        let selection = match Select::new("Select a position to configure:", options).prompt() {
            Ok(s) => s,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Err(e) => return Err(e.into()),
        };

        if selection.starts_with("✓") {
            break;
        }

        // Strip the summary suffix to get the bare position name.
        let position = selection.split_whitespace().next().unwrap_or("").to_string();
        edit_position(config, &position)?;
    }

    config.save()?;
    println!();
    println!("  ✓ Configuration saved to config.toml");
    println!();
    Ok(())
}

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Build the top-level position list, appending the current pick order as a hint.
fn position_menu_options(prefs: &LanePreferences) -> Vec<String> {
    let positions = [
        ("Top", &prefs.top),
        ("Jungle", &prefs.jungle),
        ("Mid", &prefs.mid),
        ("Bot", &prefs.bot),
        ("Support", &prefs.support),
        ("Fill", &prefs.fill),
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

    options.push(SAVE_EXIT.to_string());
    options
}

/// Interactive editor for a single position's champion list.
fn edit_position(config: &mut Config, position: &str) -> Result<()> {
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

        let mut actions = vec!["Add champion".to_string()];
        if !list.is_empty() {
            actions.push("Remove champion".to_string());
            if list.len() > 1 {
                actions.push("Move champion up".to_string());
                actions.push("Move champion down".to_string());
            }
        }
        actions.push(BACK.to_string());

        let action =
            match Select::new("What would you like to do?", actions).prompt() {
                Ok(a) => a,
                Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
                Err(e) => return Err(e.into()),
            };

        match action.as_str() {
            "Add champion" => action_add(config, position)?,
            "Remove champion" => action_remove(config, position)?,
            "Move champion up" => action_move(config, position, -1)?,
            "Move champion down" => action_move(config, position, 1)?,
            _ => break, // Back
        }
    }
    Ok(())
}

fn action_add(config: &mut Config, position: &str) -> Result<()> {
    let name = match Text::new("Champion name to add:").prompt() {
        Ok(n) => n.trim().to_string(),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if name.is_empty() {
        println!("  (empty name — skipped)");
        return Ok(());
    }

    let list = champions_for_position_mut(config, position);
    if list.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
        println!("  '{name}' is already in the list.");
    } else {
        list.push(name.clone());
        println!("  Added '{name}'.");
    }
    Ok(())
}

fn action_remove(config: &mut Config, position: &str) -> Result<()> {
    let list = champions_for_position_mut(config, position);
    let options = list.clone();

    let chosen = match Select::new("Remove which champion?", options).prompt() {
        Ok(c) => c,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let list = champions_for_position_mut(config, position);
    list.retain(|c| c != &chosen);
    println!("  Removed '{chosen}'.");
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

fn champions_for_position_mut<'a>(config: &'a mut Config, position: &str) -> &'a mut Vec<String> {
    match position {
        "Top" => &mut config.preferences.top,
        "Jungle" => &mut config.preferences.jungle,
        "Mid" => &mut config.preferences.mid,
        "Bot" => &mut config.preferences.bot,
        "Support" => &mut config.preferences.support,
        _ => &mut config.preferences.fill,
    }
}
