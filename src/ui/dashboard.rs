use dioxus::prelude::*;

use crate::app_state::AppState;
use super::components::{
    activity_log::ActivityLog,
    phase_card::PhaseCard,
};

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
