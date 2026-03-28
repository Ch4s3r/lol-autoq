# UI E2E Tests Design

**Date:** 2026-03-28
**Status:** Approved

---

## Goal

Add end-to-end UI tests that exercise the full user-facing flow (lobby → ready check → champion select → ban → pick → lock-in) using mocked LCU JSON data from a real captured session. Playwright drives a real browser; a Rust integration test binary orchestrates everything from a single `cargo test --test e2e`.

---

## Architecture Overview

```
tests/
  e2e/
    main.rs                        ← integration test entry point
    mock_lcu.rs                    ← axum mock LCU server
    harness.rs                     ← process lifecycle (dx serve + playwright)
    scenarios/
      lobby_to_ready_check.rs
      champion_select_ban_and_pick.rs
    playwright/
      lobby_to_ready_check.spec.ts
      champion_select_ban_and_pick.spec.ts
    fixtures/                      ← committed real LCU JSON payloads
      gameflow_none.json
      gameflow_readycheck.json
      gameflow_champselect.json
      champselect_session_ban_phase.json
      champselect_session_pick_phase.json
      champion_summary.json
      ddragon_versions.json

src/
  bin/
    web.rs                         ← thin web entry point (feature = "web")
    extract_fixtures.rs            ← one-shot log parser, writes fixtures/
  main.rs                          ← desktop entry (unchanged)
  lcu.rs                           ← LCU_BASE_URL read from env var (new)
```

**Per-test flow:**
1. Rust test binary binds `127.0.0.1:0` → starts `axum` mock LCU on that port
2. Sets `LCU_BASE_URL=http://127.0.0.1:<mock_port>` in env, spawns `dx serve --platform web --features web` as a child process
3. Polls `http://localhost:8080` until ready (max 30s)
4. Invokes `npx playwright test <scenario>.spec.ts` via `std::process::Command`
5. Asserts exit code 0; both child processes killed in `Drop`

---

## Dual-Target Build

The `desktop` feature stays exactly as-is. A `web` feature flag enables the browser target.

**`Cargo.toml` changes:**
```toml
[features]
desktop = ["dioxus/desktop"]
web = ["dioxus/web"]

[dependencies]
dioxus = { version = "0.7", default-features = false }

[dev-dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }

[[bin]]
name = "lol-autoq"
path = "src/main.rs"
required-features = ["desktop"]

[[bin]]
name = "lol-autoq-web"
path = "src/bin/web.rs"
required-features = ["web"]

[[bin]]
name = "extract-fixtures"
path = "src/bin/extract_fixtures.rs"
```

**`src/bin/web.rs`:**
```rust
fn main() {
    dioxus::launch(lol_autoq::ui::App);
}
```

**`src/lcu.rs` change** — replace hardcoded base URL with env-var override:
```rust
let base = std::env::var("LCU_BASE_URL")
    .unwrap_or_else(|_| format!("https://127.0.0.1:{}", lockfile.port));
```

Additionally, the lockfile discovery must be bypassed when `LCU_BASE_URL` is set — the mock does not produce a lockfile. A sentinel env var `LCU_MOCK=1` skips lockfile parsing and uses `LCU_BASE_URL` directly.

---

## Mock LCU Server

An `axum` server defined in `tests/e2e/mock_lcu.rs`. It serves a scripted phase sequence controlled by the test.

**State:**
```rust
struct MockState {
    phase_sequence: Vec<String>,       // e.g. ["None", "Matchmaking", "ReadyCheck", "ChampSelect"]
    phase_index: Arc<AtomicUsize>,     // incremented on each /gameflow-phase call
    recorded_calls: Arc<Mutex<Vec<RecordedCall>>>,
    fixtures_dir: PathBuf,
}

enum RecordedCall { AcceptQueue, HoverChampion { action_id: u64, champion_id: i64 }, LockIn { action_id: u64 } }
```

**Endpoints:**

| Endpoint | Behaviour |
|---|---|
| `GET /lol-gameflow/v1/gameflow-phase` | Returns `phase_sequence[index]`, increments index (clamps at last) |
| `POST /lol-matchmaking/v1/ready-check/accept` | Records call, returns `{}` |
| `GET /lol-champ-select/v1/session` | Returns fixture JSON matching current phase |
| `PATCH /lol-champ-select/v1/session/actions/:id` | Records hover call with champion_id from body |
| `POST /lol-champ-select/v1/session/actions/:id/complete` | Records lock-in call |
| `GET /lol-game-data/assets/v1/champion-summary.json` | Returns `fixtures/champion_summary.json` |
| `GET /api/versions.json` (ddragon) | Returns `["14.8.1"]` inline — the ddragon base URL is also controlled via a `DDRAGON_BASE_URL` env var, defaulting to `https://ddragon.leagueoflegends.com`; the mock serves this path too |

The mock binds to `127.0.0.1:0` (OS-assigned port) so parallel tests never conflict.

---

## Fixture Extraction

A one-shot binary `extract-fixtures` parses a `RUST_LOG=trace` log file and writes each endpoint's last captured response body to `tests/e2e/fixtures/<slug>.json`.

**Usage:**
```sh
RUST_LOG=trace cargo run -- start   # play through a full session
cargo run --bin extract-fixtures -- lol-autoq.log
```

**Parsing:** Lines matching the pattern:
```
[HH:MM:SS] [TRACE] response body (method=GET, url=…/<endpoint-path>, body=<value>)
```
The `body=` value may be a quoted string (`"ChampSelect"`) or a raw JSON object/array. The extractor handles both. The endpoint path is slugified to a filename (e.g. `lol-gameflow/v1/gameflow-phase` → `gameflow_phase.json`).

Fixtures are committed to the repo after first extraction. The extractor is only re-run when refreshing fixtures from a new session.

---

## E2E Test Scenarios

### Scenario 1: `lobby_to_ready_check`

**Phase sequence:** `None → None → Matchmaking → ReadyCheck → None`

**Playwright asserts (`lobby_to_ready_check.spec.ts`):**
- Connection chip text becomes "Connected to LCU"
- Phase card title shows "Ready Check!"
- Phase card description shows "A match was found — accepting queue…"

**Rust asserts (after Playwright exits):**
- `recorded_calls` contains exactly one `AcceptQueue`

---

### Scenario 2: `champion_select_ban_and_pick`

**Phase sequence:** `ChampSelect` held for N poll cycles (ban phase session, then pick phase session)

**Playwright asserts (`champion_select_ban_and_pick.spec.ts`):**
- Timeline panel appears with "Actions" header
- Ban card transitions from "Waiting…" → "Hovering: \<champion\>" → "Banned: \<champion\>"
- Hover card shows "Hovering: \<champion\>"
- Pick card transitions to "Locked in: \<champion\>"

**Rust asserts (after Playwright exits):**
- `recorded_calls` contains at least one `HoverChampion`
- `recorded_calls` contains exactly one `LockIn`

---

## `flake.nix` Additions

Add to `devShells.default.buildInputs`:
```nix
pkgs.nodejs_22
pkgs.playwright-driver.browsers   # chromium headless, no system install needed
```

Add env var so Playwright finds the Nix-managed browser:
```nix
PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
```

A `package.json` + `playwright.config.ts` live at the repo root. `npx playwright` is invoked by the Rust test harness — no separate `npm test` script needed for normal use.

---

## File Layout Summary

| File | Purpose |
|---|---|
| `src/bin/web.rs` | Dioxus web launch shim |
| `src/bin/extract_fixtures.rs` | One-shot log → fixture extractor |
| `src/lcu.rs` | +env-var base URL override |
| `tests/e2e/main.rs` | `cargo test --test e2e` entry |
| `tests/e2e/mock_lcu.rs` | axum mock server |
| `tests/e2e/harness.rs` | Child process lifecycle |
| `tests/e2e/scenarios/` | Per-scenario Rust test fns |
| `tests/e2e/playwright/*.spec.ts` | Playwright step definitions |
| `tests/e2e/fixtures/*.json` | Committed real LCU payloads |
| `playwright.config.ts` | Playwright config (baseURL, browser) |
| `package.json` | `@playwright/test` dependency |

---

## What Is Not In Scope

- Visual regression / screenshot diffing
- Testing the Settings page interactions (sliders, pickers) — unit tests cover the pure logic already
- Windows CI (LCU lockfile path is Windows-specific; mock bypasses it, but CI will be Linux/macOS only)
