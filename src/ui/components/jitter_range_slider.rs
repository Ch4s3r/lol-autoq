use dioxus::prelude::*;

#[component]
pub fn JitterRangeSlider(
    min_val: u64,
    max_val: u64,
    max_secs: u64,
    on_change: EventHandler<(u64, u64)>,
) -> Element {
    // Local signals own the thumb positions so that dragging one thumb
    // doesn't cause the other's value attribute to be overwritten mid-drag.
    let mut local_min = use_signal(|| min_val);
    let mut local_max = use_signal(|| max_val);

    // Keep local state in sync when the parent pushes new prop values
    // (e.g. after a config reload). Only update when the prop actually differs
    // from what we have, so we don't clobber an in-progress drag.
    if *local_min.read() != min_val {
        local_min.set(min_val);
    }
    if *local_max.read() != max_val {
        local_max.set(max_val);
    }

    let lo = *local_min.read();
    let hi = *local_max.read();

    let display = if lo == 0 && hi == 0 {
        "Off".to_string()
    } else if lo == hi {
        format!("{lo}s")
    } else {
        format!("{lo}–{hi}s")
    };

    // When both thumbs coincide, put min on top so it can be dragged right.
    let min_z = if lo >= hi { 5 } else { 3 };
    let max_z = if lo >= hi { 4 } else { 5 };

    rsx! {
        div { class: "timer-card",
            div { class: "timer-label", "Timer jitter" }
            div { class: "timer-sublabel", "Random delay added at lock-in (0 = off)" }
            div { class: "timer-value", "{display}" }
            div { class: "range-slider",
                input {
                    r#type: "range",
                    class: "range-min",
                    style: "z-index: {min_z};",
                    min: "0",
                    max: "{max_secs}",
                    value: "{lo}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            let clamped = v.min(*local_max.read());
                            local_min.set(clamped);
                            on_change.call((clamped, *local_max.read()));
                        }
                    },
                }
                input {
                    r#type: "range",
                    class: "range-max",
                    style: "z-index: {max_z};",
                    min: "0",
                    max: "{max_secs}",
                    value: "{hi}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u64>() {
                            let clamped = v.max(*local_min.read());
                            local_max.set(clamped);
                            on_change.call((*local_min.read(), clamped));
                        }
                    },
                }
            }
        }
    }
}
