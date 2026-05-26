# GW2 Legendary Overlay

A transparent always-on-top desktop overlay for Guild Wars 2. Tracks
the achievements, world bosses, items, builds, and todos you care
about, and surfaces what's *actionable right now* — boss spawning in
8 minutes, fractal-only item bound on character X, daily todo not yet
ticked, legendary you're 87% of the way through.

Personal use; minimal; configurable; works without any third-party
service (only the official GW2 API).

**Current version:** v0.1.11 — full English display, signed
auto-update, Smart Legendary recipe walker.

---

## What it does

The overlay ships as **three independent always-on-top windows** that
you can place + resize anywhere. Each can be collapsed to a thin
header strip, hidden via hotkey, or quit cleanly so positions persist.

### Main window — "GW2 Configure"

Seven tabs:

1. **Events** — world bosses + meta events feed, sorted by next spawn
   per expansion (Core / HoT / PoF / LWS3 / LWS4 / EoD / SotO / JW /
   Special). Pin a boss with `+`. Active events show "active · 12m
   left" in green; imminent (≤10 min) in orange. Waypoint chat code
   copy button on every entry. Filler phases (Idle / Reset / Prep)
   are filtered out so the next *real* event always shows.
2. **Catalog** — 32 curated legendary collections (3 trinkets,
   1 backpack, 3 armor sets, 20 Gen 1 weapons, 4 Gen 2 weapons,
   Klobjarne Geirr). Grouped by generation. Each card has a
   recipe-progress `📦 N%` badge driven by your owned inventory +
   wallet vs the curated recipe data. Expand to see top 5 missing
   leaves. "READY?" banner at ≥ 95%.
3. **Search** — name search across the 8200+ cached achievement
   definitions.
4. **Items** — search across your bank, material storage, shared
   inventory, every character's bags AND equipped slots, plus wallet
   currencies (gold formatted as `Xg Ys Zc`). Names in English.
5. **Todos** — custom daily / weekly todos with automatic reset
   (00:00 UTC daily / Mondays 07:30 UTC weekly).
6. **Builds** — curated meta-builds catalog (Snowcrows + MetaBattle)
   filterable by game mode / source / class. Click `📋 Code` to copy
   the chat-code template. Real chat codes still need in-game
   curation per build.
7. **WV** — Wizard's Vault daily / weekly / special objectives,
   reflected from `/v2/account/wizardsvault/*`.

### Bosses window

Just the boss groups from your Pinned view. Each pinned world boss /
meta event renders with its name, countdown, waypoint copy button,
and a collapsible list of linked achievements (done + to-do).

### Achievements window

Just the standalone pinned items — legendary collection steps, raid
achievements, random pins. Bits inside each achievement resolve to
English item / skin names with a 🔗 link straight to the canonical
wiki page.

### Plus

- **Auto-update** — non-modal banner on launch when a new release
  ships. One-click install + relaunch. Signed via minisign.
- **Toast notifications** when a pinned boss / meta phase is about to
  spawn (configurable lead time 1–15 min in Settings).
- **Smart urgency ranking** — a pinned achievement linked to a boss
  spawning in the next 10 minutes gets an orange-banded highlight so
  it floats to your attention. Bits inside that achievement inherit
  the highlight if still incomplete.
- **Configurable global hotkeys** — remap Ctrl+Shift+G/H/B/P from
  Settings → Hotkeys.
- **Diagnostics + feedback** — Settings panel buttons: open the
  rolling log folder, copy the last 200 lines to clipboard, file a
  bug report on GitHub (pre-filled with version + UA), feature
  request.
- **Reset window layout** + **Reset database** in Settings for
  recovery when something goes sideways.

---

## Install

Either install a signed release from
[the GitHub Releases page](https://github.com/J7U7G7/GW2-legendary-overlay/releases),
or build from source.

### From release

Download `GW2.Legendary.Overlay_X.Y.Z_x64-setup.exe` (NSIS) or
`GW2.Legendary.Overlay_X.Y.Z_x64_en-US.msi`. Windows SmartScreen will
warn since the binary isn't Authenticode-signed (no cert) — click
"More info" → "Run anyway".

After the first install, subsequent versions auto-update via the
in-app banner.

### From source

| | Version |
|---|---|
| OS | Windows 10/11 (DPAPI key storage is Windows-only) |
| Node | 18+ (tested with 24) |
| Rust | 1.95+ (install via [rustup](https://rustup.rs)) |
| Visual Studio Build Tools | 2022 with "Desktop development with C++" workload |
| WebView2 Runtime | Ships with Win11; on Win10 install via Microsoft |
| Guild Wars 2 | Must run in **windowed fullscreen** (the overlay can't render over exclusive fullscreen) |

You'll need a [GW2 API key](https://account.arena.net/applications)
with these scopes: `account, progression, unlocks, inventories,
characters, wallet`.

```powershell
git clone <this repo>
cd gw2-overlay
npm install
npm run tauri dev
```

For a release build:

```powershell
npm run tauri build
```

Installer lands in `src-tauri/target/release/bundle/{msi,nsis}/`.

### First launch

1. Three windows open. The main one asks for your API key (stored
   DPAPI-encrypted in `%APPDATA%\com.tripleseptconsulting.gw2overlay\
   gw2-overlay.sqlite`).
2. Validates against `/v2/tokeninfo`, checks scopes.
3. Backgrounds spin up:
   - Bulk-sync of all 8200+ achievement definitions (~50 s, once /
     week or after a Reset DB).
   - Account-progress sync (every 5 min).
   - Wallet sync (every 5 min).
   - Wizard's Vault sync (every 15 min).
   - Account-items sync (every 30 min).
   - Boss-spawn watcher (every 30 s for the toast notifications).

Subsequent launches restore window positions, re-hydrate the progress
snapshot, and re-validate the key tolerantly — a transient
`/v2/tokeninfo` blip no longer kicks you back to the setup screen.

---

## Default hotkeys

| Shortcut | Effect |
|---|---|
| `Ctrl+Shift+G` | Toggle main overlay visibility |
| `Ctrl+Shift+H` | Toggle click-through (overlay stops capturing clicks) |
| `Ctrl+Shift+B` | Toggle the Bosses window |
| `Ctrl+Shift+P` | Toggle the Achievements window |

All registered globally so they fire even with GW2 focused. Remap any
of them from **Settings → Hotkeys**. The click-through can leave the
overlay uninteractable; press `Ctrl+Shift+H` again from anywhere to
recover.

Quit cleanly with the **⏻ button** in the main header — saves window
positions before exiting. Pressing Ctrl+C in the `tauri dev` terminal
SIGINTs the process and the saved positions are lost.

---

## How it works (executive summary)

```
┌─────────────────────────────────────────────────────────────────┐
│ Tauri 2 process — three WebView2 windows                       │
│  ┌─────────────────┐ ┌──────────────┐ ┌─────────────────────┐  │
│  │ main            │ │ bosses       │ │ achievements        │  │
│  │ Configure tabs  │ │ Pinned boss  │ │ Pinned achievement  │  │
│  │ (Events, ...)   │ │ groups       │ │ list                │  │
│  └────────┬────────┘ └──────┬───────┘ └──────────┬──────────┘  │
│           ▼                  ▼                    ▼             │
│   Tauri IPC (typed via src/lib/tauri.ts)                       │
│   ↳ retry wrapper handles boot-time 'state not managed' race   │
│           ▼                                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Rust backend                                            │   │
│  │  api/         GW2 client + rate limiter + DPAPI key    │   │
│  │  db/          SQLite (rusqlite bundled) + 10 migrations│   │
│  │  sync/        achievements / progress / wv / inventory  │   │
│  │               / wallet / skins / items / engine         │   │
│  │  timers/      boss schedule + spawn math               │   │
│  │  scorer/      urgency ranking                          │   │
│  │  catalog/     legendary + boss-link JSON loaders       │   │
│  │  legendary.rs recipe walker + progress aggregator      │   │
│  │  builds.rs    static builds catalog loader             │   │
│  │  commands.rs  Tauri IPC handlers + emit events         │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** for the full
module tour, **[docs/PIVOTS.md](docs/PIVOTS.md)** for the five
mental-model pivots that shaped the project,
**[docs/ROADMAP.md](docs/ROADMAP.md)** for what's planned, and
**[CLAUDE.md](CLAUDE.md)** for AI-assistant onboarding.

For non-trivial features, the spec + plan workflow lives at
`docs/superpowers/specs/` and `docs/superpowers/plans/`. The
v0.1.11 full-English rollout used this end-to-end and the documents
are good reference for the next round.

---

## Extending the project

### Add a new legendary collection
Edit `src-tauri/data/legendary_collections.json`. Verify achievement
ids against
`https://api.guildwars2.com/v2/achievements/categories?ids=114,118,125,173`.

### Add a leaf to a legendary recipe
Edit `src-tauri/data/legendary_recipes.json`. Verify item ids against
`https://api.guildwars2.com/v2/items?ids=...&lang=en`. Mark `notes`
for anything not directly verifiable (e.g. account-bound precursors,
RNG-gated drops).

### Link an achievement to a boss
Edit `src-tauri/data/achievement_boss_links.json`. Boss id must match
`boss_schedule.json`.

### Add a new world boss or meta event
Edit `src-tauri/data/boss_schedule.json`. Each entry needs
`schedule_utc` (or `anchor_utc` + phases for metas) and a chat
`waypoint_code`.

### Add a build to the Builds tab
Edit `src-tauri/data/builds.json`. Copy a chat code from in-game
(Hero panel → Build template → right-click the slot → Copy Chat
Code) into the `chat_code` field.

### Add a new Tauri command
1. Write it in `src-tauri/src/commands.rs` with `#[tauri::command]`.
2. Register it in `src-tauri/src/lib.rs` inside `generate_handler!`.
3. Add a typed wrapper in `src/lib/tauri.ts` so the frontend can
   call it.
4. If it mutates state that other windows can see, emit a Tauri
   event (`pinned_changed`, `appearance_changed`, `hotkeys_changed`
   — name future events in past tense).

### Add a new database table or column
1. Append a new migration block at the end of
   `src-tauri/src/db/schema.rs`'s `MIGRATIONS` array. **Never edit a
   past migration** — it'd skew clients with older schema versions.
2. Update the `fresh_migration_creates_all_tables` test to include
   the new table.
3. Schema is at v10 — see `BILINGUAL_NAMES_SCHEMA` (v9) and
   `FULL_ENGLISH_SCHEMA` (v10) for examples of `ALTER TABLE`
   migrations.

### Add a new persisted setting
Use `db.get_setting(key)` / `db.set_setting(key, value)`. Or extend
the `AppearanceSettings` / `HotkeyConfig` struct in `commands.rs` if
it's appearance/hotkey-facing.

### Cross-window state sync
Mutations that should propagate across the three windows (pins,
appearance, hotkeys) need to:
1. Backend emits a Tauri event (see `emit_pinned_changed` /
   `emit_appearance_changed` in `commands.rs`, plus the
   `hotkeys_changed` emit in `cmd_set_hotkeys`).
2. Each window's `useCrossWindowSync` hook (or `useHotkeys` for
   shortcuts) listens and re-fetches / re-binds.

---

## Running the test suite

```powershell
# Unit tests (DB, API, sync, timers, scorer, catalog, legendary, builds)
cargo test --manifest-path src-tauri/Cargo.toml --lib                 # 67 tests

# Live-API integration tests (#[ignore]'d by default)
$env:GW2_API_KEY = "<your-key>"
cargo test --manifest-path src-tauri/Cargo.toml --test api_integration -- --ignored --nocapture

# Frontend type-check + bundle
npm run build

# Clippy with warnings as errors (enforce before each commit)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs the same on every push to main
and every PR. Releases (`.github/workflows/release.yml`) trigger on
tags matching `v*` and produce signed MSI + NSIS + `latest.json`.

---

## License & status

Personal project. No license declared; do whatever you want for
personal use. Don't redistribute as-is.

Phases 1–5 from `docs/ROADMAP.md` are done. The aspirational items
(TaCo-style pathing, mounts radial menu, Builds Manager v2, multi-
account, Linux/macOS port) are scoped but not yet implemented.
