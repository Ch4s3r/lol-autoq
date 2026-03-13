use dioxus::prelude::*;

use crate::config::INSTANT;

#[component]
pub fn TimerSlider(
    label: String,
    sublabel: String,
    value: u64,
    max_secs: u64,
    on_change: EventHandler<u64>,
) -> Element {
    let is_instant = value == INSTANT;
    let display = if is_instant {
        "Instant".to_string()
    } else {
        format!("{value}s")
    };
    let slider_val = if is_instant { max_secs } else { value.min(max_secs) };
    let value_class = if is_instant { "timer-value instant" } else { "timer-value" };

    rsx! {
        div { class: "timer-card",
            div { class: "timer-label", "{label}" }
            div { class: "timer-sublabel", "{sublabel}" }
            div { class: "{value_class}", "{display}" }

            input {
                r#type: "range",
                min: "0",
                max: "{max_secs}",
                value: "{slider_val}",
                disabled: is_instant,
                oninput: move |e| {
                    if let Ok(v) = e.value().parse::<u64>() {
                        on_change.call(v);
                    }
                },
            }

            div { class: "instant-row",
                label {
                    input {
                        r#type: "checkbox",
                        checked: is_instant,
                        onchange: move |e| {
                            if e.checked() {
                                on_change.call(INSTANT);
                            } else {
                                on_change.call(0);
                            }
                        },
                    }
                    i { class: "fa-solid fa-bolt" }
                    span { " Instant" }
                }
            }
        }
    }
}
