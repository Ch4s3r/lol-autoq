use dioxus::prelude::*;

use crate::app_state::AppState;
use crate::config::INSTANT;
use super::components::{
    lane_card::LaneCard,
    timer_slider::TimerSlider,
    champion_picker::ChampionPickerModal,
};

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Picks,
    Bans,
    Timers,
}

#[component]
pub fn Settings() -> Element {
    let mut state = use_context::<AppState>();
    let mut active_tab = use_signal(|| SettingsTab::Picks);
    let mut show_toast = use_signal(|| false);

    let save_config = move || {
        let cfg = state.config.read();
        let _ = cfg.save();
        show_toast.set(true);
        // The toast hides itself via CSS animation; reset state after 2s
        // We can't easily schedule a future from a sync closure, so we rely on
        // the CSS animation (1.55s) to fade it out, then hide the node on next interaction.
    };

    rsx! {
        div { class: "content", style: "padding-top: 0; gap: 0;",

            // Tab bar
            div { class: "tab-bar",
                for (tab, label) in [
                    (SettingsTab::Picks, "Picks"),
                    (SettingsTab::Bans, "Bans"),
                    (SettingsTab::Timers, "Timers"),
                ] {
                    button {
                        key: "{label}",
                        class: if *active_tab.read() == tab { "tab-btn active" } else { "tab-btn" },
                        onclick: move |_| active_tab.set(tab),
                        "{label}"
                    }
                }
            }

            if *active_tab.read() == SettingsTab::Picks {
                PicksTab { on_save: save_config }
            } else if *active_tab.read() == SettingsTab::Bans {
                BansTab { on_save: save_config }
            } else {
                TimersTab { on_save: save_config }
            }

            // Saved toast
            if *show_toast.read() {
                div { class: "toast", "Saved ✓" }
            }
        }
    }
}

// ── Picks tab ─────────────────────────────────────────────────────────────

#[component]
fn PicksTab(on_save: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();

    let lanes: Vec<(&'static str, fn(&crate::config::LanePreferences) -> &Vec<String>)> = vec![
        ("Top",     |p| &p.top),
        ("Jungle",  |p| &p.jungle),
        ("Mid",     |p| &p.mid),
        ("Bot",     |p| &p.bot),
        ("Support", |p| &p.support),
        ("Fill",    |p| &p.fill),
    ];

    rsx! {
        div { class: "section-content",
            for (lane_name, getter) in &lanes {
                {
                    let champs = getter(&state.config.read().preferences).clone();
                    let lane_str = lane_name.to_string();
                    let on_save = on_save.clone();
                    rsx! {
                        LaneCard {
                            key: "{lane_str}",
                            lane: lane_str.clone(),
                            champions: champs,
                            on_update: move |updated: Vec<String>| {
                                let lane_lower = lane_str.to_lowercase();
                                {
                                    let mut cfg = state.config.write();
                                    match lane_lower.as_str() {
                                        "top"     => cfg.preferences.top     = updated,
                                        "jungle"  => cfg.preferences.jungle  = updated,
                                        "mid"     => cfg.preferences.mid     = updated,
                                        "bot"     => cfg.preferences.bot     = updated,
                                        "support" => cfg.preferences.support = updated,
                                        _         => cfg.preferences.fill    = updated,
                                    }
                                }
                                on_save.call(());
                            },
                        }
                    }
                }
            }
        }
    }
}

// ── Bans tab ──────────────────────────────────────────────────────────────

#[component]
fn BansTab(on_save: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();
    let mut show_picker = use_signal(|| false);

    let bans = state.config.read().bans.clone();

    rsx! {
        div { class: "section-content",
            div { class: "lane-card",
                div { class: "lane-header", "Ban Priority" }

                div { class: "champ-list",
                    for (i, ban) in bans.iter().enumerate() {
                        div {
                            key: "{i}",
                            class: "champ-item",
                            span { class: "champ-num", "{i + 1}." }
                            span { class: "champ-name", "{ban}" }
                            div { class: "champ-actions",
                                if i > 0 {
                                    button {
                                        class: "icon-btn",
                                        title: "Move up",
                                        onclick: {
                                            let on_save = on_save.clone();
                                            move |_| {
                                                let mut cfg = state.config.write();
                                                cfg.bans.swap(i - 1, i);
                                                drop(cfg);
                                                on_save.call(());
                                            }
                                        },
                                        "↑"
                                    }
                                }
                                if i + 1 < bans.len() {
                                    button {
                                        class: "icon-btn",
                                        title: "Move down",
                                        onclick: {
                                            let on_save = on_save.clone();
                                            move |_| {
                                                let mut cfg = state.config.write();
                                                cfg.bans.swap(i, i + 1);
                                                drop(cfg);
                                                on_save.call(());
                                            }
                                        },
                                        "↓"
                                    }
                                }
                                button {
                                    class: "icon-btn danger",
                                    title: "Remove",
                                    onclick: {
                                        let on_save = on_save.clone();
                                        move |_| {
                                            state.config.write().bans.remove(i);
                                            on_save.call(());
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
                    "+ Add ban"
                }
            }

            if *show_picker.read() {
                ChampionPickerModal {
                    title: "Add Ban".to_string(),
                    current: bans.clone(),
                    on_toggle: {
                        let on_save = on_save.clone();
                        move |name: String| {
                            let mut cfg = state.config.write();
                            if let Some(pos) = cfg.bans.iter().position(|n| n == &name) {
                                cfg.bans.remove(pos);
                            } else {
                                cfg.bans.push(name);
                            }
                            drop(cfg);
                            on_save.call(());
                        }
                    },
                    on_close: move |_| show_picker.set(false),
                }
            }
        }
    }
}

// ── Timers tab ────────────────────────────────────────────────────────────

#[component]
fn TimersTab(on_save: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();

    let lock_ban  = state.config.read().lock_in_ban_secs;
    let lock_pick = state.config.read().lock_in_pick_secs;
    let hover     = state.config.read().hover_pick_secs;
    let queue     = state.config.read().accept_queue_delay_secs;
    let jitter    = state.config.read().timer_jitter_secs;

    // Non-INSTANT values must fit in 0-30s for the slider
    const MAX_SECS: u64 = 30;

    rsx! {
        div { class: "section-content",
            TimerSlider {
                label: "Ban lock-in".to_string(),
                sublabel: "Lock in the ban when this many seconds remain".to_string(),
                value: lock_ban,
                max_secs: MAX_SECS,
                on_change: {
                    let on_save = on_save.clone();
                    move |v| {
                        state.config.write().lock_in_ban_secs = v;
                        on_save.call(());
                    }
                },
            }
            TimerSlider {
                label: "Pick lock-in".to_string(),
                sublabel: "Lock in the pick when this many seconds remain".to_string(),
                value: lock_pick,
                max_secs: MAX_SECS,
                on_change: {
                    let on_save = on_save.clone();
                    move |v| {
                        state.config.write().lock_in_pick_secs = v;
                        on_save.call(());
                    }
                },
            }
            TimerSlider {
                label: "Pick hover".to_string(),
                sublabel: "Show champion hover when this many seconds remain (Instant = immediately)".to_string(),
                value: hover,
                max_secs: MAX_SECS,
                on_change: {
                    let on_save = on_save.clone();
                    move |v| {
                        state.config.write().hover_pick_secs = v;
                        on_save.call(());
                    }
                },
            }
            TimerSlider {
                label: "Queue accept delay".to_string(),
                sublabel: "Wait this many seconds before accepting a queue pop".to_string(),
                value: queue,
                max_secs: MAX_SECS,
                on_change: {
                    let on_save = on_save.clone();
                    move |v| {
                        state.config.write().accept_queue_delay_secs = v;
                        on_save.call(());
                    }
                },
            }

            // Jitter slider — never Instant, always 0-15s
            div { class: "timer-card",
                div { class: "timer-label", "Timer jitter" }
                div { class: "timer-sublabel", "Random extra delay (0 = off)" }
                div { class: "timer-value", "{jitter}s" }
                input {
                    r#type: "range",
                    min: "0",
                    max: "15",
                    value: "{jitter}",
                    oninput: {
                        let on_save = on_save.clone();
                        move |e: Event<FormData>| {
                            if let Ok(v) = e.value().parse::<u64>() {
                                state.config.write().timer_jitter_secs = v;
                                on_save.call(());
                            }
                        }
                    },
                }
            }
        }
    }
}
