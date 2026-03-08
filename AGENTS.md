# Project Guidelines

## Build and Run

```sh
cargo build                          # debug build
cargo build --release                # release build (binary at target/release/lol-autoq)
cargo run -- start                   # run the auto-queue loop
cargo run -- configure               # interactive champion/ban/timer config TUI
RUST_LOG=trace cargo run -- start    # verbose logging (every HTTP call + decisions)
```

No tests exist yet. After adding a dependency, run `cargo upgrade incompatible`.

## Architecture

Rust CLI (edition 2024) that polls the local LCU HTTPS API to auto-accept queues and pick/ban champions per lane priority lists. Six modules in `src/`:

| Module | Responsibility |
|---|---|
| `main.rs` | CLI dispatch, reconnect loop, poll state machine (phase-based intervals: 100ms active, 2s post-game, 30s in-game) |
| `lcu.rs` | Lockfile parsing (nom), reqwest HTTPS client (self-signed TLS), all API endpoints + data models |
| `champion_select.rs` | Ban/pick priority resolution, hover immediately, lock-in on timer threshold, re-check availability before locking |
| `config.rs` | TOML config: per-lane champion lists, ban list, lock-in timers. `INSTANT = u64::MAX` sentinel for immediate lock-in |
| `configure.rs` | Interactive TUI (inquire crate): edit picks/bans/timers. Falls back to free-text when client offline |
| `cli.rs` | clap derive: `start` and `configure` subcommands |

## LCU API

For endpoint discovery and schema lookups use: **https://lcu.vivide.re/**

Auth: lockfile at `C:\Riot Games\League of Legends\lockfile` (format `Name:PID:PORT:PASSWORD:PROTOCOL`), parsed with nom. Basic auth `riot:{password}`, TLS validation disabled.

Endpoints used:
- `GET /lol-gameflow/v1/gameflow-phase` — current game state string
- `POST /lol-lobby-team-builder/v1/ready-check/accept` — accept queue pop
- `GET /lol-champ-select/v1/session` — champ select session (actions, teams, bans, timer)
- `PATCH /lol-champ-select/v1/session/actions/{id}` — hover or set champion
- `POST /lol-champ-select/v1/session/actions/{id}/complete` — lock in
- `GET /lol-game-data/assets/v1/champion-summary.json` — champion id/name/alias list

## Code Conventions

- `anyhow::Result` with `.context()` on all fallible ops — no `unwrap`/`expect`
- `tracing` structured logging: `info!` user-visible, `trace!` debug, `warn!`/`error!` problems
- `serde` with `#[serde(rename = "camelCase")]` to match LCU JSON field names
- Lockfile parsed with `nom` combinators, not regex
- Champion lookup is case-insensitive via `.to_ascii_lowercase()`, maps both `name` and `alias`
- Config loaded once as immutable `&Config` for `start`; only `configure` mutates
- Per-phase state (`ready_check_accepted`, `ban_completed`, `champ_locked`, hover state) resets on phase transitions
- **No retries** — do not add retry loops anywhere; the poll loop itself already re-drives actions each cycle
- Champion data loading retries up to 10× with exponential backoff (sole exception — needed because the endpoint isn't available immediately on client start)
