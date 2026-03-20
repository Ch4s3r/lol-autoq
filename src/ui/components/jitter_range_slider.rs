use dioxus::document::eval;
use dioxus::prelude::*;

#[component]
pub fn JitterRangeSlider(
    min_val: u64,
    max_val: u64,
    max_secs: u64,
    on_change: EventHandler<(u64, u64)>,
) -> Element {
    let mut local_min = use_signal(|| min_val);
    let mut local_max = use_signal(|| max_val);

    // Sync from parent only when props genuinely change (e.g. config reload)
    if *local_min.read() != min_val { local_min.set(min_val); }
    if *local_max.read() != max_val { local_max.set(max_val); }

    let lo = *local_min.read();
    let hi = *local_max.read();

    let display = if lo == 0 && hi == 0 {
        "Off".to_string()
    } else if lo == hi {
        format!("{lo}s")
    } else {
        format!("{lo}–{hi}s")
    };

    // Inject JS once on mount to handle all pointer interaction imperatively.
    // JS sends {lo, hi} via dioxus.send() on every pointermove and pointerup.
    use_effect(move || {
        let script = format!(r#"
(function() {{
    const slider = document.querySelector('.range-slider');
    if (!slider || slider._jitter_init) return;
    slider._jitter_init = true;

    const MAX = {max_secs};
    let lo = {lo};
    let hi = {hi};
    let dragging = null; // 'min' | 'max'

    function pct(v) {{ return v / MAX; }}

    function thumbEl(which) {{
        return slider.querySelector('.range-' + which);
    }}

    function posFromEvent(e) {{
        const rect = slider.getBoundingClientRect();
        const padding = 10;
        const usable = rect.width - padding * 2;
        const x = Math.max(0, Math.min(e.clientX - rect.left - padding, usable));
        return Math.round((x / usable) * MAX);
    }}

    function updateThumbs() {{
        const minEl = thumbEl('min');
        const maxEl = thumbEl('max');
        if (minEl) minEl.style.left = (pct(lo) * 100) + '%';
        if (maxEl) maxEl.style.left = (pct(hi) * 100) + '%';
        // fill bar
        const fill = slider.querySelector('.range-fill');
        if (fill) {{
            fill.style.left = (pct(lo) * 100) + '%';
            fill.style.width = (pct(hi - lo) * 100) + '%';
        }}
    }}

    slider.addEventListener('pointerdown', function(e) {{
        const minEl = thumbEl('min');
        const maxEl = thumbEl('max');
        const distMin = minEl ? Math.abs(e.clientX - minEl.getBoundingClientRect().left - 10) : Infinity;
        const distMax = maxEl ? Math.abs(e.clientX - maxEl.getBoundingClientRect().left - 10) : Infinity;
        dragging = distMin <= distMax ? 'min' : 'max';
        slider.setPointerCapture(e.pointerId);
        e.preventDefault();
    }});

    slider.addEventListener('pointermove', function(e) {{
        if (!dragging) return;
        const v = posFromEvent(e);
        if (dragging === 'min') {{ lo = Math.min(v, hi); }}
        else                   {{ hi = Math.max(v, lo); }}
        updateThumbs();
        dioxus.send({{ lo, hi }});
        e.preventDefault();
    }});

    slider.addEventListener('pointerup', function(e) {{
        dragging = null;
    }});

    updateThumbs();
}})();
"#, max_secs = max_secs, lo = lo, hi = hi);
        let mut ev = eval(&script);
        spawn(async move {
            loop {
                match ev.recv::<serde_json::Value>().await {
                    Ok(v) => {
                        let new_lo = v["lo"].as_u64().unwrap_or(lo);
                        let new_hi = v["hi"].as_u64().unwrap_or(hi);
                        local_min.set(new_lo);
                        local_max.set(new_hi);
                        on_change.call((new_lo, new_hi));
                    }
                    Err(_) => break,
                }
            }
        });
    });

    rsx! {
        div { class: "timer-card",
            div { class: "timer-label", "Timer jitter" }
            div { class: "timer-sublabel", "Random delay added at lock-in (0 = off)" }
            div { class: "timer-value", "{display}" }
            div { class: "range-slider",
                div { class: "range-track" }
                div { class: "range-fill" }
                div { class: "range-thumb range-min" }
                div { class: "range-thumb range-max" }
            }
        }
    }
}
