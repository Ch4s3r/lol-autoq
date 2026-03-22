use dioxus::prelude::*;

use crate::app_state::{BanStatus, ChampSelectStatus, HoverStatus, PickStatus};

// ── Public helpers (pure, unit-testable) ──────────────────────────────────

/// Returns `(icon_class, label, css_modifier)` for a `HoverStatus`.
pub fn hover_display(status: &HoverStatus) -> (&'static str, String, &'static str) {
    match status {
        HoverStatus::Idle =>
            ("fa-solid fa-eye", "Waiting…".to_string(), "timeline-card--idle"),
        HoverStatus::NoPrefsConfigured { position } =>
            ("fa-solid fa-eye", format!("No prefs for {position}"), "timeline-card--muted"),
        HoverStatus::AllPicksExhausted { position } =>
            ("fa-solid fa-eye", format!("All picks exhausted ({position})"), "timeline-card--muted"),
        HoverStatus::WaitingToHover { champion_name } =>
            ("fa-solid fa-eye", format!("Waiting: {champion_name}"), "timeline-card--active"),
        HoverStatus::Hovering { champion_name } =>
            ("fa-solid fa-eye", format!("Hovering: {champion_name}"), "timeline-card--active"),
    }
}

// ── Public helpers (pure, unit-testable) ──────────────────────────────────

/// Returns `(icon_class, label, css_modifier)` for a `BanStatus`.
///
/// CSS modifiers: `timeline-card--idle`, `timeline-card--muted`,
///                `timeline-card--active`, `timeline-card--done`
pub fn ban_display(status: &BanStatus) -> (&'static str, String, &'static str) {
    match status {
        BanStatus::Idle =>
            ("fa-solid fa-ban", "Waiting…".to_string(), "timeline-card--idle"),
        BanStatus::NoBansConfigured =>
            ("fa-solid fa-ban", "No bans configured".to_string(), "timeline-card--muted"),
        BanStatus::AllBansExhausted =>
            ("fa-solid fa-ban", "All bans exhausted".to_string(), "timeline-card--muted"),
        BanStatus::WaitingToLock { champion_name } =>
            ("fa-solid fa-ban", format!("Locking: {champion_name}"), "timeline-card--active"),
        BanStatus::Hovering { champion_name } =>
            ("fa-solid fa-ban", format!("Hovering: {champion_name}"), "timeline-card--active"),
        BanStatus::Banned { champion_name } =>
            ("fa-solid fa-ban", format!("Banned: {champion_name}"), "timeline-card--done"),
    }
}

/// Returns `(icon_class, label, css_modifier)` for a `PickStatus`.
pub fn pick_display(status: &PickStatus) -> (&'static str, String, &'static str) {
    match status {
        PickStatus::Idle =>
            ("fa-solid fa-wand-magic-sparkles", "Waiting…".to_string(), "timeline-card--idle"),
        PickStatus::WaitingToLock { champion_name } =>
            ("fa-solid fa-wand-magic-sparkles", format!("Locking: {champion_name}"), "timeline-card--active"),
        PickStatus::LockedIn { champion_name } =>
            ("fa-solid fa-wand-magic-sparkles", format!("Locked in: {champion_name}"), "timeline-card--done"),
    }
}

/// Maps an LCU sub-phase string to a human-readable label.
pub fn phase_label(sub_phase: &str) -> &'static str {
    match sub_phase {
        "PLANNING"     => "Planning",
        "BAN_PICK"     => "Ban / Pick",
        "FINALIZATION" => "Finalization",
        ""             => "—",
        _              => "In Progress",
    }
}

/// Returns true when the status is considered "active" (timer pill should show).
fn hover_is_active(status: &HoverStatus) -> bool {
    matches!(
        status,
        HoverStatus::WaitingToHover { .. } | HoverStatus::Hovering { .. }
    )
}

fn ban_is_active(status: &BanStatus) -> bool {
    matches!(
        status,
        BanStatus::WaitingToLock { .. } | BanStatus::Hovering { .. }
    )
}

fn pick_is_active(status: &PickStatus) -> bool {
    matches!(status, PickStatus::WaitingToLock { .. })
}

// ── Sub-components ────────────────────────────────────────────────────────

#[component]
fn HoverCard(status: HoverStatus, time_left_secs: f64) -> Element {
    let (icon, label, modifier) = hover_display(&status);
    let show_timer = hover_is_active(&status);
    let secs = time_left_secs.ceil() as u64;

    rsx! {
        div { class: "timeline-card {modifier}",
            i { class: "timeline-card-icon {icon}" }
            div { class: "timeline-card-body",
                span { class: "timeline-card-type", "Hover" }
                span { class: "timeline-card-label", "{label}" }
            }
            if show_timer {
                span { class: "timeline-timer-pill", "{secs}s" }
            }
        }
    }
}

#[component]
fn BanCard(status: BanStatus, time_left_secs: f64) -> Element {
    let (icon, label, modifier) = ban_display(&status);
    let show_timer = ban_is_active(&status);
    let secs = time_left_secs.ceil() as u64;

    rsx! {
        div { class: "timeline-card {modifier}",
            i { class: "timeline-card-icon {icon}" }
            div { class: "timeline-card-body",
                span { class: "timeline-card-type", "Ban" }
                span { class: "timeline-card-label", "{label}" }
            }
            if show_timer {
                span { class: "timeline-timer-pill", "{secs}s" }
            }
        }
    }
}

#[component]
fn PickCard(status: PickStatus, time_left_secs: f64) -> Element {
    let (icon, label, modifier) = pick_display(&status);
    let show_timer = pick_is_active(&status);
    let secs = time_left_secs.ceil() as u64;

    rsx! {
        div { class: "timeline-card {modifier}",
            i { class: "timeline-card-icon {icon}" }
            div { class: "timeline-card-body",
                span { class: "timeline-card-type", "Pick" }
                span { class: "timeline-card-label", "{label}" }
            }
            if show_timer {
                span { class: "timeline-timer-pill", "{secs}s" }
            }
        }
    }
}

// ── Public component ──────────────────────────────────────────────────────

#[component]
pub fn ActionTimeline(status: ChampSelectStatus) -> Element {
    let secs = status.time_left_secs.ceil() as u64;

    rsx! {
        div { class: "timeline-panel",
            p { class: "timeline-header", "Actions" }
            div { class: "timeline-phase-row",
                span { class: "timeline-sub-phase", "{phase_label(&status.sub_phase)}" }
                span { class: "timeline-countdown", "{secs}s" }
            }
            div { class: "timeline-cards",
                HoverCard { status: status.hover, time_left_secs: status.time_left_secs }
                BanCard   { status: status.ban,   time_left_secs: status.time_left_secs }
                PickCard  { status: status.pick,  time_left_secs: status.time_left_secs }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ban_display_idle_shows_waiting_label_and_idle_modifier() {
        let (_, label, modifier) = ban_display(&BanStatus::Idle);
        assert!(label.contains("Waiting"), "label should mention waiting, got: {label}");
        assert_eq!(modifier, "timeline-card--idle");
    }

    #[test]
    fn ban_display_hovering_includes_champion_name_and_active_modifier() {
        let (_, label, modifier) = ban_display(&BanStatus::Hovering { champion_name: "Zed".into() });
        assert!(label.contains("Zed"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--active");
    }

    #[test]
    fn ban_display_waiting_to_lock_shows_active_modifier() {
        let (_, label, modifier) = ban_display(&BanStatus::WaitingToLock { champion_name: "Yasuo".into() });
        assert!(label.contains("Yasuo"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--active");
    }

    #[test]
    fn ban_display_banned_includes_champion_name_and_done_modifier() {
        let (_, label, modifier) = ban_display(&BanStatus::Banned { champion_name: "Yone".into() });
        assert!(label.contains("Yone"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--done");
    }

    #[test]
    fn ban_display_no_bans_configured_shows_muted_modifier() {
        let (_, _, modifier) = ban_display(&BanStatus::NoBansConfigured);
        assert_eq!(modifier, "timeline-card--muted");
        let (_, _, modifier2) = ban_display(&BanStatus::AllBansExhausted);
        assert_eq!(modifier2, "timeline-card--muted");
    }

    #[test]
    fn hover_display_idle_shows_waiting_label_and_idle_modifier() {
        let (_, label, modifier) = hover_display(&HoverStatus::Idle);
        assert!(label.contains("Waiting"), "label should mention waiting, got: {label}");
        assert_eq!(modifier, "timeline-card--idle");
    }

    #[test]
    fn hover_display_hovering_includes_champion_name_and_active_modifier() {
        let (_, label, modifier) = hover_display(&HoverStatus::Hovering { champion_name: "Ahri".into() });
        assert!(label.contains("Ahri"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--active");
    }

    #[test]
    fn hover_display_waiting_to_hover_shows_active_modifier() {
        let (_, label, modifier) = hover_display(&HoverStatus::WaitingToHover { champion_name: "Jinx".into() });
        assert!(label.contains("Jinx"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--active");
    }

    #[test]
    fn hover_display_no_prefs_and_exhausted_show_muted_modifier() {
        let (_, _, modifier) = hover_display(&HoverStatus::NoPrefsConfigured { position: "Mid".into() });
        assert_eq!(modifier, "timeline-card--muted");
        let (_, _, modifier2) = hover_display(&HoverStatus::AllPicksExhausted { position: "Bot".into() });
        assert_eq!(modifier2, "timeline-card--muted");
    }

    #[test]
    fn pick_display_idle_shows_waiting_label_and_idle_modifier() {
        let (_, label, modifier) = pick_display(&PickStatus::Idle);
        assert!(label.contains("Waiting"), "label should mention waiting, got: {label}");
        assert_eq!(modifier, "timeline-card--idle");
    }

    #[test]
    fn pick_display_locked_in_includes_champion_name_and_done_modifier() {
        let (_, label, modifier) = pick_display(&PickStatus::LockedIn { champion_name: "Jinx".into() });
        assert!(label.contains("Jinx"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--done");
    }

    #[test]
    fn pick_display_waiting_to_lock_shows_active_modifier() {
        let (_, label, modifier) = pick_display(&PickStatus::WaitingToLock { champion_name: "Ahri".into() });
        assert!(label.contains("Ahri"), "label should include champion name, got: {label}");
        assert_eq!(modifier, "timeline-card--active");
    }

    #[test]
    fn phase_label_maps_all_known_sub_phases_and_unknown_fallback() {
        assert_eq!(phase_label("PLANNING"),     "Planning");
        assert_eq!(phase_label("BAN_PICK"),     "Ban / Pick");
        assert_eq!(phase_label("FINALIZATION"), "Finalization");
        assert_eq!(phase_label(""),             "—");
        assert_eq!(phase_label("SOMETHING_NEW"), "In Progress");
    }
}
