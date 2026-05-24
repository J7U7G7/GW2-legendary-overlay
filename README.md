# GW2 Overlay

A transparent always-on-top desktop overlay for Guild Wars 2. Tracks the
achievements, world bosses, items, builds, and todos you care about, and
surfaces what's *actionable right now* — boss spawning in 8 minutes,
fractal-only item bound on character X, daily todo not yet ticked.

Personal use; minimal; configurable; works without any third-party
service (only the official GW2 API).

---

## What it does

The overlay ships as **three independent always-on-top windows** that
you can place + resize anywhere. Each can be collapsed to a thin header
strip, hidden via hotkey, or quit cleanly so positions persist.

### Main window — "GW2 Configure"
Seven tabs:

1. **Events** — world bosses + meta events feed, sorted by next spawn
   per expansion (Core / HoT / PoF / LWS3 / LWS4 / EoD / SotO / JW /
   Special). Pin a boss with `+`. Active events show "active · 12m left"
   in green; imminent (≤10 min) in orange. Waypoint chat code copy
   button on every entry.
2. **Catalog** — 32 curated legendary collections (trinkets, backs,
   armors, all 20 Gen 1 weapons, 4 Gen 2 weapons, Klobjarne Geirr).
   Grouped by generation, filter by kind. Each card links to the wiki
   recipe page.
3. **Search** — name search across the 8 200+ cached GW2 achievements.
4. **Items** — search across your *own* bank, material storage, shared
   inventory, every character's bags **and equipped slots**. Item names
   are in French (toggleable in the future). One-click resync.
5. **Todos** — custom daily / weekly todos with automatic reset
   (00:00 UTC daily / Mondays 07:30 UTC weekly).
6. **Builds** — curated meta builds catalog with one-click chat-code
   copy. Ships placeholders; you replace the chat codes with real ones
   from Snowcrows / your hero panel.
7. **WV** — Wizard's Vault daily / weekly / special objectives,
   reflected from `/v2/account/wizardsvault/*`.

### Bosses window
Just the boss groups from your Pinned view. Each pinned world boss /
meta event renders with its name, countdown ("in 8m" / "active · 12m
left"), waypoint copy button, and a collapsible list of linked
achievements (done + to-do).

### Achievements window
Just the standalone pinned items — legendary collection steps, raid
achievements, random pins. Sorted with pending first, done at the
bottom with reduced opacity.

### Plus

- **Toast notifications** when a pinned boss / meta phase is about to
  spawn (configurable lead time 1–15 min in Settings).
- **Smart urgency ranking** — a pinned achievement linked to a boss
  spawning in the next 10 minutes gets an orange-banded highlight so it
  floats to your attention. Bits inside that achievement inherit the
  highlight if they're still incomplete.

---

## Requirements

| | Version |
|---|---|
| OS | Windows 10/11 (DPAPI-based key storage is Windows-only) |
| Node | 18+ (tested with 25) |
| Rust | 1.95+ (install via [rustup](https://rustup.rs)) |
| Visual Studio Build Tools | 2022 with "Desktop development with C++" workload |
| WebView2 Runtime | Ships with Win11; on Win10 install via Microsoft |
| Guild Wars 2 | Must run in **windowed fullscreen** (the overlay can't render over exclusive fullscreen) |

You also need a [GW2 API key](https://account.arena.net/applications)
with these scopes: `account, progression, unlocks, inventories,
characters, wallet`.

---

## Quick start

```powershell
git clone <this repo>
cd gw2-overlay
npm install
npm run tauri dev
```

First launch:

1. Three windows open. The main one asks for your API key (stored
   DPAPI-encrypted in `%APPDATA%\com.tripleseptconsulting.gw2overlay\
   gw2-overlay.sqlite`).
2. Validates against `/v2/tokeninfo`, checks scopes.
3. Backgrounds:
   - Bulk-sync of all 8 200+ achievement definitions (~50 s, once / week).
   - Account-progress sync (every 5 min).
   - Wizard's Vault sync (every 15 min).
   - Account-items sync (every 30 min).
   - Boss-spawn watcher (every 30 s for the toast notifications).

Subsequent launches restore window positions, re-hydrate the progress
snapshot, and re-validate the key tolerantly — a transient
`/v2/tokeninfo` blip no longer kicks you back to the setup screen.

For a release build:

```powershell
npm run tauri build
```

Installer lands in `src-tauri/target/release/bundle/msi/`.

---

## Default hotkeys

| Shortcut | Effect |
|---|---|
| `Ctrl+Shift+G` | Toggle main overlay visibility |
| `Ctrl+Shift+H` | Toggle click-through (overlay stops capturing clicks) |
| `Ctrl+Shift+B` | Toggle the Bosses window |
| `Ctrl+Shift+P` | Toggle the Achievements window |

All registered globally so they fire even with GW2 focused. The
click-through can leave the overlay uninteractable; press
`Ctrl+Shift+H` again from anywhere to recover.

Quit cleanly with the **⏻ button** in the main header — saves window
positions before exiting. Pressing Ctrl+C in the `tauri dev` terminal
SIGINTs the process and the saved positions are lost.

---

## How it works (executive summary)

```
┌─────────────────────────────────────────────────────────────────┐
│ Tauri 2 process — three webview windows                        │
│  ┌─────────────────┐ ┌──────────────┐ ┌─────────────────────┐  │
│  │ main            │ │ bosses       │ │ achievements        │  │
│  │ Configure tabs  │ │ Pinned boss  │ │ Pinned achievement  │  │
│  │ (Events, ...)   │ │ groups       │ │ list                │  │
│  └────────┬────────┘ └──────┬───────┘ └──────────┬──────────┘  │
│           ▼                  ▼                    ▼             │
│   Tauri IPC (typed via src/lib/tauri.ts)                       │
│           ▼                                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Rust backend                                            │   │
│  │  api/      GW2 client + rate limiter + DPAPI key       │   │
│  │  db/       SQLite (rusqlite bundled) + 6 migrations    │   │
│  │  sync/     achievements / progress / wv / inventory /   │   │
│  │            items / engine                               │   │
│  │  timers/   boss schedule + spawn math                  │   │
│  │  scorer/   urgency ranking                             │   │
│  │  catalog/  legendary + boss-link JSON loaders          │   │
│  │  builds.rs static builds catalog loader                │   │
│  │  commands.rs — Tauri IPC handlers                      │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** for the full
module tour, **[docs/PIVOTS.md](docs/PIVOTS.md)** for the four
mental-model pivots that shaped the project,
**[docs/ROADMAP.md](docs/ROADMAP.md)** for what's planned but not yet
built (Smart Legendary Selector, TaCo-style pathing, mounts radial,
configurable hotkeys), and **[CLAUDE.md](CLAUDE.md)** for AI-assistant
onboarding.

---

## Extending the project

### Add a new legendary collection
Edit `src-tauri/data/legendary_collections.json`. Verify ids against
`https://api.guildwars2.com/v2/achievements/categories?ids=114,118,125,173`.

### Link an achievement to a boss
Edit `src-tauri/data/achievement_boss_links.json`. Boss id must match
`boss_schedule.json`.

### Add a new world boss or meta event
Edit `src-tauri/data/boss_schedule.json`. Each entry needs
`schedule_utc` (or `anchor_utc` + phases for metas) and a chat
`waypoint_code`. The Python helper `scripts/update_meta_events.py`
shows how to bulk-replace the meta_events array.

### Add a build to the Builds tab
Edit `src-tauri/data/builds.json`. Copy a chat code from in-game (Hero
panel → Build template → right-click the slot → Copy Chat Code) into
the `chat_code` field.

### Add a new Tauri command
1. Write it in `src-tauri/src/commands.rs` with `#[tauri::command]`.
2. Register it in `src-tauri/src/lib.rs` inside `generate_handler!`.
3. Add a typed wrapper in `src/lib/tauri.ts` so the frontend can call
   it.

### Add a new database table
1. Append a new migration block at the end of
   `src-tauri/src/db/schema.rs`'s `MIGRATIONS` array. **Never edit a
   past migration.**
2. Update the `fresh_migration_creates_all_tables` test to include the
   new table name.

### Add a new persisted setting
Use `db.get_setting(key)` / `db.set_setting(key, value)`. Or extend the
`AppearanceSettings` struct in `commands.rs` if it's appearance-facing.

### Cross-window state sync
Mutations that should propagate across the three windows (pins,
appearance, etc.) need to:
1. Backend emits a Tauri event (see `emit_pinned_changed` /
   `emit_appearance_changed` in `commands.rs`).
2. Each window's `useCrossWindowSync` hook listens and re-fetches.

---

## Running the test suite

```powershell
# Unit tests (DB, API, sync, timers, scorer, catalog, builds)
cargo test --manifest-path src-tauri/Cargo.toml --lib                  # 56+ tests

# Live-API integration tests (#[ignore]'d by default)
$env:GW2_API_KEY = "<your-key>"
cargo test --manifest-path src-tauri/Cargo.toml --test api_integration -- --ignored --nocapture

# Frontend type-check + bundle
npm run build

# Clippy with warnings as errors (enforce before each commit)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

---

## License & status

Personal project. No license declared; do whatever you want for
personal use. Don't redistribute as-is.

Phase 1 of the spec is fully done. Phase 2 partially shipped (pinning,
legendary catalog, item search, todos, builds infra). The aspirational
items in [docs/ROADMAP.md](docs/ROADMAP.md) (Smart Legendary Selector,
TaCo-style pathing, mounts radial menu, configurable hotkeys) are
scoped but not yet implemented.
