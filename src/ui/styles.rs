/// Material Design 3 dark theme CSS, injected into the webview at startup.
pub const CSS: &str = r#"
/* ── Reset ──────────────────────────────────────────────────────────── */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

:root {
  --bg:           #1C1B1F;
  --surface:      #2B2930;
  --surf-var:     #3A3840;
  --outline:      #49454F;
  --primary:      #D0BCFF;
  --on-primary:   #381E72;
  --secondary:    #CCC2DC;
  --on-surface:   #E6E1E5;
  --on-surf-var:  #CAC4D0;
  --muted:        #938F99;
  --success:      #A8D5A2;
  --warning:      #FFD966;
  --error:        #F2B8B5;
  --radius-sm:    8px;
  --radius-md:    12px;
  --radius-lg:    16px;
}

html, body, #main {
  height: 100%;
  background: var(--bg);
  color: var(--on-surface);
  font-family: 'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 14px;
  line-height: 1.5;
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
}

/* ── App shell ───────────────────────────────────────────────────────── */
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

/* ── Top bar ────────────────────────────────────────────────────────── */
.top-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 16px 12px;
  background: var(--surface);
  border-bottom: 1px solid var(--outline);
  flex-shrink: 0;
}
.top-bar-icon { font-size: 18px; }
.top-bar-title {
  font-size: 17px;
  font-weight: 700;
  color: var(--primary);
  letter-spacing: 0.01em;
}
.top-bar-version {
  font-size: 11px;
  color: var(--muted);
  margin-left: auto;
}

/* ── Scrollable content ─────────────────────────────────────────────── */
.content {
  flex: 1;
  overflow-y: auto;
  padding: 14px 14px 8px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* ── Bottom nav ─────────────────────────────────────────────────────── */
.bottom-nav {
  display: flex;
  background: var(--surface);
  border-top: 1px solid var(--outline);
  flex-shrink: 0;
}
.nav-btn {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 10px 8px 12px;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--muted);
  font-size: 11px;
  font-weight: 500;
  transition: color 0.2s;
  -webkit-user-select: none;
  user-select: none;
}
.nav-btn:hover { color: var(--on-surf-var); }
.nav-btn.active { color: var(--primary); }
.nav-btn-icon {
  font-size: 20px;
  line-height: 1;
  transition: transform 0.2s;
}
.nav-btn.active .nav-btn-icon { transform: scale(1.1); }

/* ── Connection chip ────────────────────────────────────────────────── */
.chip {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 5px 12px;
  border-radius: 20px;
  font-size: 12px;
  font-weight: 500;
  width: fit-content;
}
.chip-connected {
  background: rgba(168,213,162, 0.12);
  color: var(--success);
  border: 1px solid rgba(168,213,162, 0.25);
}
.chip-searching {
  background: rgba(208,188,255, 0.10);
  color: var(--secondary);
  border: 1px solid rgba(208,188,255, 0.20);
}
.chip-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.chip-dot-connected { background: var(--success); }
.chip-dot-searching {
  background: var(--secondary);
  animation: pulse-dot 1.6s ease-in-out infinite;
}

/* ── Phase card ─────────────────────────────────────────────────────── */
.phase-card {
  border-radius: var(--radius-lg);
  padding: 22px 20px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 150px;
  justify-content: center;
  animation: card-enter 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
  position: relative;
  overflow: hidden;
}
.phase-card::before {
  content: '';
  position: absolute;
  inset: 0;
  background: rgba(255,255,255,0.03);
  border-radius: inherit;
}
.phase-icon {
  font-size: 30px;
  line-height: 1;
  margin-bottom: 2px;
}
.phase-title { font-size: 22px; font-weight: 800; letter-spacing: -0.02em; }
.phase-desc { font-size: 13px; opacity: 0.75; margin-top: 2px; }
.phase-champ {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  padding: 4px 10px;
  background: rgba(0,0,0,0.25);
  border-radius: 20px;
  font-size: 12px;
  color: rgba(255,255,255,0.85);
  width: fit-content;
}

/* Phase gradients */
.phase-disconnected { background: linear-gradient(135deg, #252429 0%, #2d2b32 100%); }
.phase-lobby        { background: linear-gradient(135deg, #1a2a5e 0%, #253580 100%); }
.phase-matchmaking  { background: linear-gradient(135deg, #2d1b69 0%, #3d2490 100%); animation: card-enter 0.35s cubic-bezier(0.34,1.56,0.64,1), phase-breathe 2.8s ease-in-out infinite; }
.phase-readycheck   { background: linear-gradient(135deg, #7c2d12 0%, #c2410c 100%); animation: card-enter 0.35s cubic-bezier(0.34,1.56,0.64,1), phase-pulse 0.9s ease-in-out infinite; }
.phase-champselect  { background: linear-gradient(135deg, #064e3b 0%, #047857 100%); }
.phase-ingame       { background: linear-gradient(135deg, #7f1d1d 0%, #991b1b 100%); }
.phase-endgame      { background: linear-gradient(135deg, #1a1830 0%, #252440 100%); }

/* ── Activity log ───────────────────────────────────────────────────── */
.activity-log {
  background: var(--surface);
  border-radius: var(--radius-md);
  padding: 12px;
  overflow: hidden;
  flex-shrink: 0;
}
.activity-header {
  font-size: 10px;
  font-weight: 700;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  margin-bottom: 8px;
}
.activity-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
  max-height: 160px;
  overflow-y: auto;
  overflow-anchor: auto;
}
.activity-list > :last-child {
  overflow-anchor: auto;
}
.activity-entry {
  display: flex;
  gap: 8px;
  padding: 3px 0;
  font-size: 12px;
  animation: fade-in 0.25s ease;
}
.activity-time { color: var(--muted); min-width: 54px; flex-shrink: 0; }
.activity-info    { color: var(--on-surf-var); }
.activity-success { color: var(--success); }
.activity-warning { color: var(--warning); }
.activity-empty   { color: var(--muted); font-style: italic; font-size: 12px; }

/* ── Settings tabs ──────────────────────────────────────────────────── */
.tab-bar {
  display: flex;
  border-bottom: 1px solid var(--outline);
  margin-bottom: 14px;
  flex-shrink: 0;
}
.tab-btn {
  flex: 1;
  padding: 11px 12px;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  cursor: pointer;
  color: var(--muted);
  font-size: 13px;
  font-weight: 600;
  transition: color 0.2s, border-color 0.2s;
  -webkit-user-select: none;
}
.tab-btn:hover { color: var(--on-surf-var); }
.tab-btn.active { color: var(--primary); border-bottom-color: var(--primary); }

/* ── Lane / ban card ────────────────────────────────────────────────── */
.lane-card {
  background: var(--surface);
  border-radius: var(--radius-md);
  padding: 12px 14px;
}
.lane-header {
  font-size: 10px;
  font-weight: 700;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  margin-bottom: 8px;
}
.champ-list { display: flex; flex-direction: column; }
.champ-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 0;
  border-bottom: 1px solid rgba(73,69,79,0.4);
}
.champ-item:last-child { border-bottom: none; }
.champ-item[draggable]:hover { background: rgba(73,69,79,0.3); border-radius: var(--radius-sm); }
.champ-list-icon {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  object-fit: cover;
  object-position: center top;
  flex-shrink: 0;
  background: var(--outline);
}
.champ-drag-handle {
  color: var(--outline);
  font-size: 12px;
  cursor: grab;
  padding: 0 4px;
  flex-shrink: 0;
  transition: color 0.15s;
}
.champ-item:hover .champ-drag-handle { color: var(--muted); }
.champ-num { color: var(--muted); min-width: 18px; font-size: 11px; font-weight: 600; }
.champ-name { flex: 1; font-size: 13px; color: var(--on-surface); }
.champ-actions { display: flex; gap: 2px; }
.icon-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--muted);
  padding: 2px 5px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  transition: color 0.15s, background 0.15s;
  line-height: 1.4;
  -webkit-user-select: none;
}
.icon-btn:hover { color: var(--on-surface); background: var(--surf-var); }
.icon-btn.danger:hover { color: var(--error); }
.add-champ-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  margin-top: 8px;
  padding: 7px 12px;
  background: none;
  border: 1px dashed var(--outline);
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--secondary);
  font-size: 12px;
  font-weight: 500;
  transition: all 0.2s;
  -webkit-user-select: none;
}
.add-champ-btn:hover {
  background: rgba(208,188,255,0.06);
  border-color: var(--primary);
  color: var(--primary);
}

/* ── Timer sliders ──────────────────────────────────────────────────── */
.timer-card {
  background: var(--surface);
  border-radius: var(--radius-md);
  padding: 14px 16px;
}
.timer-label { font-size: 13px; font-weight: 600; color: var(--on-surface); }
.timer-sublabel { font-size: 11px; color: var(--muted); margin-top: 1px; }
.timer-value {
  font-size: 26px;
  font-weight: 800;
  color: var(--primary);
  line-height: 1;
  margin: 8px 0 4px;
}
.timer-value.instant { color: var(--success); }
input[type=range] {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 4px;
  border-radius: 2px;
  background: var(--outline);
  outline: none;
  margin: 10px 0 8px;
  cursor: pointer;
}
input[type=range]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--primary);
  cursor: pointer;
  box-shadow: 0 0 0 4px rgba(208,188,255,0.18);
  transition: box-shadow 0.15s;
}
input[type=range]:hover::-webkit-slider-thumb { box-shadow: 0 0 0 6px rgba(208,188,255,0.25); }
input[type=range]:disabled { opacity: 0.35; cursor: not-allowed; }
input[type=range]:disabled::-webkit-slider-thumb { cursor: not-allowed; }
.instant-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}
.instant-row label {
  font-size: 12px;
  color: var(--secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
}
input[type=checkbox] {
  width: 15px; height: 15px;
  accent-color: var(--primary);
  cursor: pointer;
}

/* ── Champion picker modal ───────────────────────────────────────────── */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.65);
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  z-index: 500;
  animation: fade-in 0.18s ease;
}
.picker-sheet {
  background: var(--surface);
  border-radius: var(--radius-lg) var(--radius-lg) 0 0;
  padding: 16px 14px 24px;
  display: flex;
  flex-direction: column;
  max-height: 80vh;
  animation: slide-up 0.28s cubic-bezier(0.34,1.56,0.64,1);
}
.picker-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.picker-title { font-size: 15px; font-weight: 700; color: var(--on-surface); }
.picker-close {
  background: var(--surf-var);
  border: none;
  cursor: pointer;
  color: var(--muted);
  width: 30px; height: 30px;
  border-radius: 50%;
  font-size: 16px;
  display: flex; align-items: center; justify-content: center;
  transition: color 0.15s, background 0.15s;
}
.picker-close:hover { color: var(--on-surface); background: var(--outline); }
.picker-search {
  width: 100%;
  padding: 9px 14px;
  background: var(--surf-var);
  border: 1px solid var(--outline);
  border-radius: var(--radius-sm);
  color: var(--on-surface);
  font-size: 13px;
  outline: none;
  margin-bottom: 12px;
  transition: border-color 0.15s;
}
.picker-search:focus { border-color: var(--primary); }
.picker-search::placeholder { color: var(--muted); }
.champion-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
  overflow-y: auto;
  flex: 1;
  padding-right: 2px;
}
.champ-tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 8px 4px 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  border: 2px solid transparent;
  transition: background 0.15s, border-color 0.15s;
  position: relative;
  background: rgba(73,69,79,0.2);
  -webkit-user-select: none;
}
.champ-tile:hover { background: var(--surf-var); }
.champ-tile.selected {
  border-color: var(--primary);
  background: rgba(208,188,255,0.1);
}
.champ-portrait {
  width: 54px;
  height: 54px;
  border-radius: 50%;
  object-fit: cover;
  object-position: center top;
  background: var(--outline);
  display: block;
}
.champ-tile-name {
  font-size: 10px;
  text-align: center;
  color: var(--on-surf-var);
  line-height: 1.2;
  max-width: 70px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.champ-tile.selected .champ-tile-name { color: var(--primary); }
.tile-check {
  position: absolute;
  top: 3px;
  right: 3px;
  width: 16px; height: 16px;
  background: var(--primary);
  color: var(--on-primary);
  border-radius: 50%;
  font-size: 9px;
  font-weight: 800;
  display: flex; align-items: center; justify-content: center;
}

/* ── Toast ──────────────────────────────────────────────────────────── */
.toast {
  position: fixed;
  bottom: 74px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--surf-var);
  color: var(--success);
  padding: 7px 18px;
  border-radius: 20px;
  font-size: 12px;
  font-weight: 600;
  z-index: 200;
  pointer-events: none;
  animation: toast-show 0.2s ease, toast-hide 0.25s ease 1.3s forwards;
}

/* ── Section headers ────────────────────────────────────────────────── */
.section-content { display: flex; flex-direction: column; gap: 8px; }

/* ── Scrollbar ──────────────────────────────────────────────────────── */
::-webkit-scrollbar { width: 4px; height: 4px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--outline); border-radius: 2px; }
::-webkit-scrollbar-thumb:hover { background: var(--muted); }

/* ── Animations ─────────────────────────────────────────────────────── */
@keyframes card-enter {
  from { opacity: 0; transform: translateY(18px) scale(0.97); }
  to   { opacity: 1; transform: translateY(0)    scale(1); }
}
@keyframes phase-breathe {
  0%, 100% { filter: brightness(1); }
  50%       { filter: brightness(1.1); }
}
@keyframes phase-pulse {
  0%, 100% { filter: brightness(1) saturate(1); }
  50%       { filter: brightness(1.15) saturate(1.3); }
}
@keyframes pulse-dot {
  0%, 100% { opacity: 1;   transform: scale(1); }
  50%       { opacity: 0.4; transform: scale(0.7); }
}
@keyframes fade-in {
  from { opacity: 0; }
  to   { opacity: 1; }
}
@keyframes slide-up {
  from { transform: translateY(100%); }
  to   { transform: translateY(0); }
}
@keyframes toast-show {
  from { opacity: 0; transform: translateX(-50%) translateY(8px); }
  to   { opacity: 1; transform: translateX(-50%) translateY(0); }
}
@keyframes toast-hide {
  from { opacity: 1; }
  to   { opacity: 0; pointer-events: none; }
}
"#;
