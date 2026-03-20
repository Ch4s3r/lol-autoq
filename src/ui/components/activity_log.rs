use dioxus::prelude::*;

use crate::app_state::ActivityEntry;

#[component]
pub fn ActivityLog(entries: Vec<ActivityEntry>) -> Element {
    rsx! {
        div { class: "activity-log",
            p { class: "activity-header", "Activity" }
            div { class: "activity-list",
                if entries.is_empty() {
                    p { class: "activity-empty", "No activity yet" }
                }
                for (i, entry) in entries.iter().rev().enumerate() {
                    div {
                        key: "{i}",
                        class: "activity-entry",

                        span { class: "activity-time", "{entry.timestamp}" }
                        span {
                            class: "{entry.kind.css_class()}",
                            "{entry.message}"
                        }
                    }
                }
                div { class: "activity-anchor" }
            }
        }
    }
}
