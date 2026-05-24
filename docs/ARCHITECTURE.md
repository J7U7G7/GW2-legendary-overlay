# Architecture

This is the technical deep-dive. For a user-facing intro see
[README.md](../README.md). For the history of how we got here see
[PIVOTS.md](PIVOTS.md).

## Top-down view

```
┌────────────────────────────────────────────────────────────────┐
│ Tauri 2 process                                                │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ WebView (Edge / WebView2 on Windows)                     │  │
│  │   React 19 + Zustand + TailwindCSS v3                    │  │
│  │   - App.tsx routes by window label                       │  │
│  │   - Overlay (main window): Pinned / Catalog / Search /WV │  │
│  │   - EventsWindow (events label): boss + meta feed only   │  │
│  │   - SettingsPanel: opacity, accent color, font size      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                       ▲                                        │
│                       │ tauri::invoke (typed via lib/tauri.ts) │
│                       ▼                                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Rust backend                                             │  │
│  │   api/        — GW2 client + DPAPI key + rate limiter   │  │
│  │   db/         — SQLite schema/migrations + repository   │  │
│  │   sync/       — engine, achievements, progress, WV, items│  │
│  │   timers/     — boss schedule + spawn math              │  │
│  │   scorer/     — weighted urgency ranking                │  │
│  │   catalog/    — static legendary + boss-link JSON load  │  │
│  │   commands.rs — Tauri IPC handlers                      │  │
│  │   error.rs    — AppError enum (serializable to JS)      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                │
└────────────────────────────────────────────────────────────────┘
                       │
                       ▼
            GW2 official API (no third-party services)
```

The two-window layout (since commit `a612816`) is achieved by declaring
both windows in `src-tauri/tauri.conf.json` with distinct labels (`main`
and `events`), each loading `index.html` with a different URL fragment.
`src/App.tsx` reads `getCurrentWindow().label` and renders either
`<Overlay/>` or `<EventsWindow/>`.

## Crate / module layout

```
src-tauri/
├── Cargo.toml             — Rust deps (Tauri 2, rusqlite, reqwest, etc.)
├── tauri.conf.json        — window declarations, identifier, capabilities
├── capabilities/
│   └── default.json       — Tauri 2 permission list (which commands JS can call)
├── data/
│   ├── boss_schedule.json — world bosses + meta events + ley-line
│   ├── achievement_boss_links.json — achievement_id → boss_id metadata
│   └── legendary_collections.json  — curated legendary catalog
└── src/
    ├── lib.rs             — Tauri Builder + setup hook + state mgmt
    ├── main.rs            — entry point (delegates to lib::run)
    ├── error.rs           — AppError + Result alias
    ├── api/
    │   ├── auth.rs        — ApiKey newtype + DPAPI encrypt/decrypt
    │   ├── client.rs      — reqwest client + token-bucket rate limiter
    │   └── endpoints.rs   — typed wrappers for /v2/* endpoints
    ├── catalog/
    │   └── mod.rs         — JSON loaders for legendary + boss-link catalogs
    ├── commands.rs        — all #[tauri::command] IPC handlers
    ├── db/
    │   ├── schema.rs      — versioned migration list + migrate()
    │   └── repository.rs  — Db struct (Mutex<Connection>) + helpers
    ├── scorer/
    │   └── ranking.rs     — Weights + Scoreable + rank()
    ├── sync/
    │   ├── engine.rs      — SyncEngine (boss-watcher, periodic loops)
    │   ├── achievements.rs— bulk paginated definition sync
    │   ├── progress.rs    — /v2/account/achievements + diff
    │   ├── wizardsvault.rs— WV daily/weekly/special
    │   └── items.rs       — items_cache fetch + lookup
    └── timers/
        ├── schedule.rs    — WorldBoss / MetaEvent / Schedule structs + load
        └── engine.rs      — prev_spawn / next_spawn / current_meta_phase /
                             all_upcoming

src/
├── App.tsx                — routes by window label
├── main.tsx               — ReactDOM root
├── components/
│   ├── Overlay.tsx        — main window shell (header, tabs, footer)
│   ├── ApiKeySetup.tsx    — first-launch form
│   ├── PinnedPanel.tsx    — boss groups + standalone pins + per-bit detail
│   ├── EventsTab.tsx      — events list grouped by expansion (inside main)
│   ├── EventsWindow.tsx   — events feed in its own window
│   ├── CatalogView.tsx    — legendary catalog grouped by generation
│   ├── SearchView.tsx     — name search across cached achievements
│   ├── WizardsVaultPanel.tsx — WV objectives (legacy daily/weekly/special)
│   └── SettingsPanel.tsx  — opacity / colors / font size sliders
├── hooks/
│   └── useHotkeys.ts      — global shortcuts (Ctrl+Shift+G/H/E)
├── lib/
│   ├── tauri.ts           — typed wrappers around invoke('cmd_…')
│   └── format.ts          — stripGw2Markup, fillRequirement,
│                            eventTimeLabel, wikiUrl
├── store/
│   ├── app.ts             — primary Zustand store (data + actions)
│   └── settings.ts        — appearance Zustand store
├── types/
│   └── gw2.ts             — TS mirrors of every Rust IPC payload
└── styles/
    └── tailwind.css       — base + CSS variables for live theming
```

## Data flow: a single user action

When the user clicks "+ Pin" on an achievement in the Catalog tab:

1. React `onClick` → `useAppStore.pin(id, collectionKey)`
2. `pin()` calls `invoke("cmd_pin_achievement", { achievementId, collectionKey })`
3. Tauri serialises to the Rust command in `commands.rs::cmd_pin_achievement`
4. The command calls `db.pin_achievement(id, collection_key)`
5. SQLite `INSERT OR IGNORE INTO pinned_achievements (...)` runs
6. Command returns `Ok(())`, JS promise resolves
7. `pin()` then calls `await get().refresh()`
8. `refresh()` triggers parallel `getWizardsVaultState / getProgressSummary
   / getPinnedView` calls
9. Each command queries SQLite, builds a `PinnedView` struct, sends back
10. Zustand stores the new state, React re-renders Pinned + Catalog
11. `pin()` then calls `api.warmItemCache()` so any Item-typed bits in the
    new achievement get fetched from `/v2/items` and cached
12. If `warmItemCache` returns > 0, `pin()` calls `refresh()` again so the
    item names appear

Steps 7-12 form the "post-mutation refresh" pattern used by every pin /
unpin / boss-group action.

## State storage

There's only one persistent store: SQLite. It lives at
`%APPDATA%\com.tripleseptconsulting.gw2overlay\gw2-overlay.sqlite` on
Windows (the path is `app_data_dir()` from Tauri's path resolver).

Tables (current schema version 4):

| Table | Origin | Purpose |
|---|---|---|
| `_migrations` | bootstrap | tracks applied migration versions |
| `achievements` | bulk sync | full definition cache (~8 200 rows) |
| `account_progress` | progress sync | per-achievement current/max/done/bits |
| `daily_assignments` | (unused — legacy) | reserved for `/v2/achievements/daily` which is deprecated |
| `wizardsvault` | WV sync | (period_type, period_start, objective_id) rows |
| `settings` | manual | key/value: API key blob, appearance JSON, last_full_sync timestamp |
| `achievement_metadata` | catalog load | `associated_boss`, `tags`, etc. for our own enrichment |
| `pinned_achievements` | user | user's pinned achievement ids |
| `pinned_bosses` | user | user's pinned world boss ids |
| `legendary_collections` | catalog load | curated catalog of legendaries |
| `legendary_collection_members` | catalog load | join table (collection_key, achievement_id, step) |
| `items_cache` | items sync | id → name + description for Item-typed bits |

The two boolean stores in the runtime — the **progress snapshot** (in-RAM
`HashMap<u32, AccountAchievement>` used to compute diffs) and the
**notified set** (in-RAM `HashSet<(boss_id, spawn_time)>` for de-duping
notifications) — are rebuilt at boot from SQLite and live in
`SyncEngine`. They're not persisted; they don't need to be.

## Sync engine lifecycle

`SyncEngine` is created in the Tauri `setup` hook **iff** a valid API
key is already stored, and it's recreated by `cmd_set_api_key` whenever
the user enters a new key. It spawns four background tokio tasks via
`tauri::async_runtime::spawn`:

1. **Achievements bootstrap** — one-shot, runs at engine start. Skips if
   `settings.achievements_last_full_sync < 7 days` ago. Otherwise pulls
   `/v2/achievements?page=N&page_size=200` until exhausted (~42 pages for
   ~8 200 achievements) and upserts the definitions.
2. **Progress loop** — `tokio::time::interval(300s)` ticks every 5
   minutes, pulls `/v2/account/achievements`, diffs against the in-RAM
   snapshot (NewlyDone / Progressed / NewlyUnlocked changes), persists,
   refreshes snapshot.
3. **Wizard's Vault loop** — `interval(900s)`, hits daily/weekly/special
   in sequence, persists per-period rows keyed by `(period_type,
   period_start, objective_id)`.
4. **Boss watcher loop** — `interval(30s)`. For each row in
   `pinned_bosses`, computes `next_spawn(boss, now)`; if the next spawn
   is within 2 minutes and we haven't notified for that `(boss_id,
   spawn_time)` pair, fires a Windows toast via
   `tauri-plugin-notification`. Garbage-collects expired pairs after 1h.

All loops use `tokio_util::CancellationToken` so a key rotation (which
calls `engine.shutdown()`) makes them exit cleanly on the next tick.

## Window-state plugin oddity

`tauri-plugin-window-state` 2.4 does NOT auto-restore window position on
its own — you have to call `WindowExt::restore_state(StateFlags::all())`
explicitly in `setup`. We do this for both windows (`main` and `events`)
in a loop. The plugin's `Builder::default()` only registers the save-on-
close hook, not the restore-on-open.

`tauri.conf.json` declares window `width`/`height` but **not** `x`/`y` —
the plugin's restore overrides the position on subsequent launches, and
the first-ever launch defaults to OS-positioning (centered or whatever).

## Rate limiting

`api::client::ApiClient` uses a token-bucket: capacity 300 tokens
refilled at 5 tok/sec (300 per minute, 50 % below the documented
600/min limit). Every authenticated `GET` waits for a token. On a 429
or 5xx response we sleep with exponential backoff (1, 2, 4, 8 s, max
30) and retry up to 4 times. After that the call returns
`AppError::RateLimited` (true 429s exhausted) or
`AppError::Unavailable(status)` (5xx exhausted) so the UI can show a
meaningful error instead of just "network error".

## Why these tech choices

- **Tauri 2 vs Electron** — ~20 MB binary vs ~200 MB, native
  transparent always-on-top window, lower CPU at rest, single Rust
  binary for the sync engine.
- **SQLite (rusqlite bundled)** vs file-based JSON — atomic
  transactions for sync, indexed lookups for the 8 200-row
  achievements table, no DLL dependency thanks to the bundled feature.
- **Zustand** vs Redux — same shape but much less boilerplate, fits a
  single-process app without ceremony.
- **Tailwind** vs styled-components — CSS variables can be patched
  at runtime by the Settings panel without round-tripping through React
  state for every key/value.
- **DPAPI** vs Stronghold — Windows-native, no master password
  needed, no extra crate cost. Trades cross-platform (cross-machine
  keys don't decrypt across user accounts — which is what we want for
  a personal overlay).

## Known constraints / non-goals

- The overlay is **Windows-only by design**. DPAPI key storage,
  WebView2 dependency, and the lack of a Linux/macOS Tauri-build
  pipeline put this firmly in personal-tool territory.
- It requires GW2 in **windowed fullscreen**. Exclusive fullscreen
  renders the overlay invisible; we don't detect this yet but the
  README warns about it.
- No multi-account support — there's exactly one API key stored at a
  time.
- No system-tray icon. Closing the main window quits the app
  (this is the intentional default behavior; closing the secondary
  events window only hides it).
