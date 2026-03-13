use dioxus::prelude::*;

use super::champion_picker::ChampionPickerModal;

#[component]
pub fn LaneCard(
    lane: String,
    champions: Vec<String>,
    on_update: EventHandler<Vec<String>>,
) -> Element {
    let mut show_picker = use_signal(|| false);

    rsx! {
        div { class: "lane-card",
            div { class: "lane-header", "{lane}" }

            div { class: "champ-list",
                for (i, champ) in champions.iter().enumerate() {
                    div {
                        key: "{i}",
                        class: "champ-item",

                        span { class: "champ-num", "{i + 1}." }
                        span { class: "champ-name", "{champ}" }

                        div { class: "champ-actions",
                            // Move up
                            if i > 0 {
                                button {
                                    class: "icon-btn",
                                    title: "Move up",
                                    onclick: {
                                        let mut champs = champions.clone();
                                        let on_update = on_update;
                                        move |_| {
                                            champs.swap(i - 1, i);
                                            on_update.call(champs.clone());
                                        }
                                    },
                                    "↑"
                                }
                            }
                            // Move down
                            if i + 1 < champions.len() {
                                button {
                                    class: "icon-btn",
                                    title: "Move down",
                                    onclick: {
                                        let mut champs = champions.clone();
                                        let on_update = on_update;
                                        move |_| {
                                            champs.swap(i, i + 1);
                                            on_update.call(champs.clone());
                                        }
                                    },
                                    "↓"
                                }
                            }
                            // Remove
                            button {
                                class: "icon-btn danger",
                                title: "Remove",
                                onclick: {
                                    let mut champs = champions.clone();
                                    let on_update = on_update;
                                    move |_| {
                                        champs.remove(i);
                                        on_update.call(champs.clone());
                                    }
                                },
                                "×"
                            }
                        }
                    }
                }
            }

            button {
                class: "add-champ-btn",
                onclick: move |_| show_picker.set(true),
                "+ Add champion"
            }

            if *show_picker.read() {
                ChampionPickerModal {
                    title: format!("Add to {lane}"),
                    current: champions.clone(),
                    on_toggle: {
                        let mut champs = champions.clone();
                        let on_update = on_update;
                        move |name: String| {
                            if let Some(pos) = champs.iter().position(|n| n == &name) {
                                champs.remove(pos);
                            } else {
                                champs.push(name);
                            }
                            on_update.call(champs.clone());
                        }
                    },
                    on_close: move |_| show_picker.set(false),
                }
            }
        }
    }
}
