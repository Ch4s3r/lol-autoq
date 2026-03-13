use dioxus::prelude::*;

use crate::app_state::GamePhase;

#[component]
pub fn PhaseCard(phase: GamePhase, hovered_champion: Option<String>) -> Element {
    let css_class = phase.css_class();
    let icon = phase.icon();
    let title = phase.label().to_string();
    let desc = phase.description().to_string();

    rsx! {
        div {
            class: "phase-card {css_class}",

            span { class: "phase-icon", i { class: "{icon}" } }
            h2 { class: "phase-title", "{title}" }
            p { class: "phase-desc",   "{desc}" }

            if let Some(champ) = hovered_champion {
                div { class: "phase-champ",
                    span { "Hovering: " }
                    strong { "{champ}" }
                }
            }
        }
    }
}
