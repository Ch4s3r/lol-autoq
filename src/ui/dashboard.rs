use dioxus::prelude::*;

use crate::app_state::{AppState, ConnectionState};
use super::components::{
    activity_log::ActivityLog,
    phase_card::PhaseCard,
};

#[component]
pub fn Dashboard() -> Element {
    let state = use_context::<AppState>();

    let connection = state.connection.read();
    let phase = state.phase.read().clone();
    let activities: Vec<_> = state.activities.read().iter().cloned().collect();
    let hovered = state.hovered_champion.read().clone();

    let (chip_class, dot_class, conn_label) = if connection.is_connected() {
        (
            "chip chip-connected",
            "chip-dot chip-dot-connected",
            connection.label().to_string(),
        )
    } else {
        (
            "chip chip-searching",
            "chip-dot chip-dot-searching",
            connection.label().to_string(),
        )
    };

    rsx! {
        div { class: "content",
            // Connection chip
            div { class: "{chip_class}",
                span { class: "{dot_class}" }
                span { "{conn_label}" }
            }

            // Phase card
            PhaseCard {
                phase: phase,
                hovered_champion: hovered,
            }

            // Activity log
            ActivityLog { entries: activities }
        }
    }
}
