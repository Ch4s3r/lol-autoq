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
            div { class: "range-slider",
                input {
                    r#type: "range",
                    class: "range-min",
                    min: "0",
                    max: "{max_secs}",
                    value: "{min_val}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            on_change.call((v.min(max_val), max_val));
                        }
                    },
                }
                input {
                    r#type: "range",
                    class: "range-max",
                    min: "0",
                    max: "{max_secs}",
                    value: "{max_val}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            on_change.call((min_val, v.max(min_val)));
                        }
                    },
                }
            }
        }
    }
}
