use dioxus::prelude::*;

#[component]
pub fn JitterRangeSlider(
    min_val: u64,
    max_val: u64,
    max_secs: u64,
    on_change: EventHandler<(u64, u64)>,
) -> Element {
    let display = if min_val == 0 && max_val == 0 {
        "Off".to_string()
    } else if min_val == max_val {
        format!("{min_val}s")
    } else {
        format!("{min_val}–{max_val}s")
    };

    rsx! {
        div { class: "timer-card",
            div { class: "timer-label", "Timer jitter" }
            div { class: "timer-sublabel", "Random delay added at lock-in (0 = off)" }
            div { class: "timer-value", "{display}" }

            div { class: "jitter-row",
                span { class: "jitter-bound-label", "Min" }
                input {
                    r#type: "range",
                    class: "jitter-min",
                    min: "0",
                    max: "{max_secs}",
                    value: "{min_val}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            // min cannot exceed max
                            on_change.call((v.min(max_val), max_val));
                        }
                    },
                }
                span { class: "jitter-bound-val", "{min_val}s" }
            }

            div { class: "jitter-row",
                span { class: "jitter-bound-label", "Max" }
                input {
                    r#type: "range",
                    class: "jitter-max",
                    min: "0",
                    max: "{max_secs}",
                    value: "{max_val}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            // max cannot go below min
                            on_change.call((min_val, v.max(min_val)));
                        }
                    },
                }
                span { class: "jitter-bound-val", "{max_val}s" }
            }
        }
    }
}
