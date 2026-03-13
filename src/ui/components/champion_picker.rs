use dioxus::prelude::*;

use crate::app_state::AppState;
use super::champion_tile::ChampionTile;

#[component]
pub fn ChampionPickerModal(
    title: String,
    current: Vec<String>,
    on_toggle: EventHandler<String>,
    on_close: EventHandler<()>,
) -> Element {
    let state = use_context::<AppState>();
    let mut query = use_signal(|| String::new());

    let summaries = state.champion_summaries.read();
    let version = state.ddragon_version.read().clone();
    let q = query.read().to_lowercase();

    let filtered: Vec<_> = summaries
        .iter()
        .filter(|c| c.id > 0 && !c.name.is_empty())
        .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.alias.to_lowercase().contains(&q))
        .collect();

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |e| {
                e.stop_propagation();
                on_close.call(());
            },

            div {
                class: "picker-sheet",
                onclick: move |e| e.stop_propagation(),

                // Header
                div { class: "picker-header",
                    span { class: "picker-title", "{title}" }
                    button {
                        class: "picker-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }

                // Search
                input {
                    class: "picker-search",
                    r#type: "text",
                    placeholder: "Search champions…",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                    autofocus: true,
                }

                // Grid
                if filtered.is_empty() {
                    p { class: "activity-empty", style: "padding: 16px 0; text-align: center;",
                        "No champions found"
                    }
                } else {
                    div { class: "champion-grid",
                        for champ in filtered {
                            ChampionTile {
                                key: "{champ.id}",
                                name: champ.name.clone(),
                                alias: champ.alias.clone(),
                                ddragon_version: version.clone(),
                                selected: current.contains(&champ.name),
                                on_click: on_toggle.clone(),
                            }
                        }
                    }
                }
            }
        }
    }
}
