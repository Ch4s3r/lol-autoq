use dioxus::prelude::*;

use crate::config::INSTANT;

/// Pure view-model for `TimerSlider` — zero I/O, fully unit-testable.
pub struct TimerSliderState {
    pub is_instant: bool,
    pub display: String,
    /// Value to feed the `<input type="range">`.
    pub slider_val: u64,
    pub value_class: &'static str,
}

/// Derives all display properties from a raw timer value and slider maximum.
pub fn timer_slider_state(value: u64, max_secs: u64) -> TimerSliderState {
    let is_instant = value == INSTANT;
    TimerSliderState {
        is_instant,
        display: if is_instant { "Instant".to_string() } else { format!("{value}s") },
        slider_val: if is_instant { max_secs } else { value.min(max_secs) },
        value_class: if is_instant { "timer-value instant" } else { "timer-value" },
    }
}

#[component]
pub fn TimerSlider(
    label: String,
    sublabel: String,
    value: u64,
    max_secs: u64,
    on_change: EventHandler<u64>,
) -> Element {
    let vm = timer_slider_state(value, max_secs);

    rsx! {
        div { class: "timer-card",
            div { class: "timer-label", "{label}" }
            div { class: "timer-sublabel", "{sublabel}" }
            div { class: "{vm.value_class}", "{vm.display}" }

            input {
                r#type: "range",
                min: "0",
                max: "{max_secs}",
                value: "{vm.slider_val}",
                disabled: vm.is_instant,
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
                        checked: vm.is_instant,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_slider_state_normal_value_below_max() {
        let vm = timer_slider_state(10, 30);
        assert!(!vm.is_instant);
        assert_eq!(vm.display, "10s");
        assert_eq!(vm.slider_val, 10);
        assert_eq!(vm.value_class, "timer-value");
    }

    #[test]
    fn timer_slider_state_value_clamped_to_max() {
        let vm = timer_slider_state(50, 30);
        assert!(!vm.is_instant);
        assert_eq!(vm.slider_val, 30, "slider_val must clamp to max_secs");
        assert_eq!(vm.display, "50s");
    }

    #[test]
    fn timer_slider_state_zero_value() {
        let vm = timer_slider_state(0, 30);
        assert!(!vm.is_instant);
        assert_eq!(vm.display, "0s");
        assert_eq!(vm.slider_val, 0);
    }

    #[test]
    fn timer_slider_state_instant_sentinel() {
        let vm = timer_slider_state(INSTANT, 30);
        assert!(vm.is_instant);
        assert_eq!(vm.display, "Instant");
        assert_eq!(vm.slider_val, 30, "instant slider should sit at max");
        assert_eq!(vm.value_class, "timer-value instant");
    }

    #[test]
    fn timer_slider_state_value_exactly_at_max() {
        let vm = timer_slider_state(30, 30);
        assert!(!vm.is_instant);
        assert_eq!(vm.slider_val, 30);
    }
}
