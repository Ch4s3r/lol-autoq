# lol-autoq

Automatically accepts ready checks and picks your preferred champion in champion select for League of Legends on **Windows**.

---

## How it works

`lol-autoq` connects to the League Client (LCU) API running locally on your machine.  
It polls for game state every 500 ms when it matters, and slows down to 30 s while a game is in progress.

| Phase | What it does |
|---|---|
| Searching / Ready Check | Instantly accepts the queue |
| Champion Select | Hovers and locks in your highest-priority available champion for your assigned lane |
| In Game | Idles (polls every 30 s) |

---

## Requirements

- League of Legends installed and running on **Windows**
- The `lol-autoq.exe` placed anywhere on the same machine

---

## Getting started

### 1. Build (from source)

```
cargo build --release
```

The binary is at `target/release/lol-autoq.exe`.

For Windows cross-compilation from another OS:

```
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

---

### 2. Configure your champion preferences

Before starting, set up which champions to play in each lane:

```
lol-autoq configure
```

You will see an interactive menu:

```
  Champion Preference Configuration

Select a position to configure:
> Top      Darius → Garen → Malphite
  Jungle   Vi → Warwick → Amumu
  Mid      Lux → Ahri → Syndra
  Bot      Jinx → Caitlyn → Jhin
  Support  Thresh → Lulu → Sona
  Fill     Garen → Lux
  ✓ Save & Exit
```

Select a lane to open its editor:

```
Mid — current pick order:
  1. Lux
  2. Ahri
  3. Syndra

What would you like to do?
> Add champion
  Remove champion
  Move champion up
  Move champion down
  ← Back
```

- **Add champion** — type any champion name (display name or in-game alias)
- **Remove champion** — pick from the current list
- **Move champion up / down** — re-order priority
- **← Back** or Esc — return to the lane selection menu
- **✓ Save & Exit** or Esc at the top level — write changes to `config.toml` and exit

The first champion in the list that is not banned or already picked will be chosen.

---

### 3. Start the auto-queue

```
lol-autoq start
```

Make sure the League of Legends client is open (or will be opened shortly).  
The tool will wait for it to start automatically.

Example output:

```
  =================================
   LoL Auto-Queue  v0.1.0
   Auto-accept queues & pick champs
  =================================

INFO Champion preferences (edit config.toml or run `lol-autoq configure` to change):
INFO   Top:     Darius -> Garen -> Malphite
INFO   Mid:     Lux -> Ahri -> Syndra
...
INFO Waiting for the League of Legends client to start...
INFO League client found — connected  port=52345
INFO Champion data loaded — ready!    count=168
INFO game state changed               phase="Searching for a match..."
INFO game state changed               phase="Ready check!"
INFO Queue accepted! Getting into champ select...
INFO game state changed               phase="Champion select"
INFO champion select                  position=Mid  pick_order="Lux -> Ahri -> Syndra"
INFO   Lux is banned or already picked — skipping
INFO   Hovering...                    champion=Ahri
INFO   Locked in!                     champion=Ahri
INFO game state changed               phase="Game is starting..."
INFO game state changed               phase="Game in progress"
```

Press **Ctrl+C** to stop.

---

## Configuration file

`config.toml` is created automatically next to the executable on first run.  
You can edit it by hand or use `lol-autoq configure`.

```toml
[preferences]
top     = ["Darius", "Garen", "Malphite"]
jungle  = ["Vi", "Warwick", "Amumu"]
mid     = ["Lux", "Ahri", "Syndra"]
bot     = ["Jinx", "Caitlyn", "Jhin"]
support = ["Thresh", "Lulu", "Sona"]
fill    = ["Garen", "Lux"]   # used when your assigned position is Fill or unknown
```

To override the lockfile path (if League is installed in a non-standard location):

```toml
lockfile_path = 'D:\Games\League of Legends\lockfile'
```

---

## Verbose / debug logging

```
set RUST_LOG=trace
lol-autoq start
```

This prints every HTTP request sent to the LCU and every decision made during champion select.

---

## Commands reference

```
Usage: lol-autoq <COMMAND>

Commands:
  start      Start the auto-queue
  configure  Interactively configure champion preferences per lane
  help       Print this message or the help for a subcommand
```
