# GW2 Overlay

A transparent always-on-top desktop overlay for Guild Wars 2 that tracks the
achievements and world bosses you care about, and highlights what's
*actionable right now*.

The overlay is for personal use — minimal, discrete, configurable, and
operates without any third-party service (only the official GW2 API).

---

## What it does

1. **Pin world bosses.** See each one's next spawn, a one-click waypoint
   chat code, and the list of related achievements (done + still to do).
2. **Pin legendary collections.** Track Aurora / Vision / Coalescence
   trinkets, Gen 1 / Gen 2 weapons, Envoy Armor, Astral Armor, Obsidian
   Tier 2, Ad Infinitum, Klobjarne Geirr — 32 collections out of the box,
   extensible via a JSON file.
3. **Pin arbitrary achievements** via name search across the 8 200+
   GW2 achievements cached locally.
4. **Smart ranking.** A pinned achievement linked to a boss spawning in the
   next 10 minutes gets a green/orange highlight so it floats to your
   attention.
5. **Windows notifications.** Toast when a pinned boss is < 2 minutes from
   spawn so you don't have to keep an eye on the timer.
6. **Click-through toggle + global hotkeys** so the overlay stops blocking
   in-game inputs when you don't need to interact with it.

---

## Requirements

| | Version |
|---|---|
| OS | Windows 10/11 (the DPAPI-based key storage is Windows-only) |
| Node | 18+ (tested with 25) |
| Rust | 1.95+ (install via [rustup](https://rustup.rs)) |
| Visual Studio Build Tools | 2022 with "Desktop development with C++" workload (Rust on Windows needs the MSVC linker) |
| WebView2 Runtime | Ships with Win11; on Win10 install via Microsoft |
| Guild Wars 2 | Run in **windowed fullscreen** (the overlay can't render over exclusive fullscreen) |

You'll also need a [GW2 API key](https://account.arena.net/applications)
with these scopes: `account, progression, unlocks, inventories, characters,
wallet`.

---

## Quick start

```powershell
git clone <this repo>
cd gw2-overlay
npm install
npm run tauri dev
```

The first launch:

1. Asks for your API key (stored encrypted via Windows DPAPI in a local
   SQLite file under `%APPDATA%\com.tripleseptconsulting.gw2overlay\`).
2. Validates it against `/v2/tokeninfo` and checks for required scopes.
3. Triggers a background bulk sync of all 8 200+ achievement definitions
   into the local cache (~50 s, only runs once per week per the
   `achievements_last_full_sync` setting).
4. Starts the periodic sync (account progress every 5 min, Wizard's Vault
   every 15 min, boss-notification watcher every 30 s).

Subsequent launches restore the window position and size, re-hydrate the
progress snapshot from the cache, and re-validate the key before doing any
network I/O.

For a release build:

```powershell
npm run tauri build
```

The installer ends up in `src-tauri/target/release/bundle/msi/`.

---

## Default hotkeys

| Shortcut | Effect |
|---|---|
| `Ctrl+Shift+G` | Toggle overlay visibility (works even with the game focused) |
| `Ctrl+Shift+H` | Toggle click-through (overlay stops capturing clicks) |
| `Ctrl+Shift+E` | Toggle the Events window |

All three are registered globally. Click-through can leave you stuck if the
overlay becomes uninteractable — press `Ctrl+Shift+H` again from anywhere
to recover.

---

## How it works (executive summary)

```
┌────────────────────────────────────────────────────────────┐
│ Tauri 2 window(s) — transparent, always-on-top, no chrome  │
│                                                            │
│  React + Zustand + TailwindCSS                             │
│  ├─ ApiKeySetup / Overlay (main window)                    │
│  └─ EventsWindow (events tab broken out into its own       │
│      always-on-top window with independent position)       │
│                ▲                                           │
│                │ Tauri IPC (invoke('cmd_…'))               │
│                ▼                                           │
│  Rust backend                                              │
│  ├─ api/        — GW2 HTTP client (reqwest + rate limiter) │
│  │              — DPAPI-encrypted key storage               │
│  ├─ db/         — SQLite (rusqlite, bundled) + migrations  │
│  ├─ sync/       — engine, achievements, progress, WV, items│
│  ├─ timers/     — boss / meta schedule + next_spawn engine │
│  ├─ scorer/     — weighted urgency ranking                 │
│  ├─ catalog/    — loaders for legendary + boss-link JSONs  │
│  └─ commands.rs — Tauri command handlers consumed by JS    │
└────────────────────────────────────────────────────────────┘
```

Read:
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — technical deep-dive
  (module layout, data flow, sync engine lifecycle, design decisions).
- **[docs/PIVOTS.md](docs/PIVOTS.md)** — history of product/architecture
  pivots during development. Three significant ones so far.
- **[docs/ROADMAP.md](docs/ROADMAP.md)** — what's planned but not built
  (3-window split, smart legendary selector, TaCo-style pathing, build
  manager, item search, todos, configurable hotkeys / notifications).
- **[CLAUDE.md](CLAUDE.md)** — house rules and pre-commit ritual, read
  by Claude Code at session start.

---

## Extending the project

These are the most common changes:

### Add a new legendary collection

Edit `src-tauri/data/legendary_collections.json` and append a `collection`
entry. Achievement IDs must be verified against the live GW2 API:

```powershell
Invoke-RestMethod -Uri "https://api.guildwars2.com/v2/achievements/categories?ids=114,118,125,173"
```

Then rebuild — the catalog loader upserts the table on every boot, so the
entries appear immediately.

### Link a boss to its achievements

Edit `src-tauri/data/achievement_boss_links.json`. Each entry maps one boss
id (matching `src-tauri/data/boss_schedule.json`) to a list of achievement
ids. The catalog loader writes these into `achievement_metadata` on boot.

```json
{ "boss_id": "claw_of_jormag", "achievement_ids": [123, 124, ...] }
```

### Add a new boss-schedule entry / fix a waypoint

Edit `src-tauri/data/boss_schedule.json`. World bosses live under
`world_bosses[]`, meta events under `meta_events[]`, and the ley-line
anomaly is its own object. Each entry has `schedule_utc` in `HH:MM` and a
`waypoint_code` chat link (verified against the wiki).

### Add a new Tauri command

1. Write it in `src-tauri/src/commands.rs` with `#[tauri::command]`.
2. Register it in `src-tauri/src/lib.rs` inside the `generate_handler!`
   macro list.
3. Add a typed wrapper in `src/lib/tauri.ts` so the frontend can call it.

### Add a new persisted setting

1. Pick a key in the `settings` table — they're just `(key, value)` rows.
2. Use `db.get_setting(key)` / `db.set_setting(key, value)` from Rust.
3. Or extend the `AppearanceSettings` struct in `commands.rs` if it's UI-
   facing — the appearance row is one JSON blob.

### Add a new database table

1. Add a SQL block at the end of `src-tauri/src/db/schema.rs`'s
   `MIGRATIONS` array. **Do not edit the existing migrations** — migrations
   are versioned and idempotent (the `_migrations` table tracks the
   applied version per row).
2. Update the `fresh_migration_creates_all_tables` test to include the new
   table name.

---

## Running the test suite

```powershell
# Unit tests (DB, API parsing, sync diff, timer math, scorer, catalog)
cargo test --manifest-path src-tauri/Cargo.toml --lib

# Integration tests — hit the live GW2 API. Marked #[ignore] so they
# don't run by default. Needs a valid GW2_API_KEY env var.
$env:GW2_API_KEY = "<your-key>"
cargo test --manifest-path src-tauri/Cargo.toml --test api_integration -- --ignored --nocapture

# Frontend type-check + bundle
npm run build

# Clippy (warnings-as-errors enforced in CI / before each commit)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The lib unit-test suite is **55 tests** (DB migrations, API parsing,
DPAPI round-trip, rate-limiter, timer rollover, scorer urgency,
catalog idempotency). The integration tests cover the GW2 endpoints we
rely on at runtime; they're how we discovered that
`/v2/achievements/daily` has been silently broken since Wizard's Vault.

---

## License & status

Personal project. No license declared; do whatever you want for personal
use. Don't redistribute as-is.

The overlay is functional but Phase-1 of the spec, with a couple of
features deferred to a later phase (configurable hotkeys from the
Settings UI, more boss-link coverage for the world bosses that have no
dedicated achievement category in the GW2 API).
