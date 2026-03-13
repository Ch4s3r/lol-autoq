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
    let mut query = use_signal(String::new);

    let summaries = state.champion_summaries.read();
    let version = state.ddragon_version.read().clone();
    let q = query.read().to_lowercase();

    let filtered: Vec<_> = summaries
        .iter()
        .filter(|c| c.is_playable())
        .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.alias.to_lowercase().contains(&q))
        .map(|c| {
            let is_selected = current.contains(&c.name);
            (c, is_selected)
        })
        .collect();

    let first_name = filtered.first().map(|(c, _)| c.name.clone());

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
                    onmounted: move |e| async move {
                        let _ = e.set_focus(true).await;
                    },
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            if let Some(name) = first_name.clone() {
                                on_toggle.call(name);
                                on_close.call(());
                            }
                        }
                    },
                }

                // Grid
                if filtered.is_empty() {
                    p { class: "activity-empty", style: "padding: 16px 0; text-align: center;",
                        "No champions found"
                    }
                } else {
                    div { class: "champion-grid",
                        for (champ, is_selected) in filtered {
                            ChampionTile {
                                key: "{champ.id}",
                                name: champ.name.clone(),
                                alias: champ.alias.clone(),
                                ddragon_version: version.clone(),
                                selected: is_selected,
                                on_click: move |name: String| {
                                    let adding = !is_selected;
                                    on_toggle.call(name);
                                    if adding {
                                        on_close.call(());
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
