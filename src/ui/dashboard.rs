use dioxus::prelude::*;

use super::components::{
    action_timeline::ActionTimeline, activity_log::ActivityLog, phase_card::PhaseCard,
};
use crate::app_state::{AppState, GamePhase};

#[component]
pub fn Dashboard() -> Element {
    let state = use_context::<AppState>();

    // Extract values immediately and drop the read guards before building RSX
    let (chip_class, dot_class, conn_label) = {
        let conn = state.connection.read();
        (
            conn.chip_class(),
            conn.dot_class(),
            conn.label().to_string(),
        )
    };
    let phase = state.phase.read().clone();
    let activities: Vec<_> = state.activities.read().iter().cloned().collect();
    let hovered = state.hovered_champion.read().clone();
    let champ_select_status = state.champ_select_status.read().clone();

    rsx! {
        div { class: "content",
            // Connection chip
            div { class: "{chip_class}",
                span { class: "{dot_class}" }
                span { "{conn_label}" }
            }

            // Phase card
            PhaseCard {
                phase: phase.clone(),
                hovered_champion: hovered,
            }

            // Action Timeline during Champion Select, Activity Log otherwise
            if let (GamePhase::ChampSelect, Some(status)) = (&phase, champ_select_status) {
                ActionTimeline { status: status }
            } else {
                ActivityLog { entries: activities }
            }
        }
    }
}
