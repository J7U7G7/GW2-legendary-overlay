# Architecture

Technical deep-dive. For a user-facing intro see [README.md](../README.md).
For the history of how we got here see [PIVOTS.md](PIVOTS.md).

## Top-down view

```
┌─────────────────────────────────────────────────────────────────┐
│ Tauri 2 process                                                 │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Three independent WebView windows (Edge / WebView2)      │   │
│  │   each loads index.html with a different URL hash:       │   │
│  │     - #main  → <Overlay/> (config + tabs)                │   │
│  │     - #bosses → <BossesWindow/> (pinned boss groups)     │   │
│  │     - #achievements → <AchievementsWindow/> (pinned ach) │   │
│  │   App.tsx routes by getCurrentWindow().label.            │   │
│  │   Each window has its OWN JS context + zustand store —   │   │
│  │   cross-window sync goes through Tauri events.           │   │
│  └──────────────────────────────────────────────────────────┘   │
│                       ▲                                         │
│                       │ tauri::invoke (typed via lib/tauri.ts)  │
│                       ▼                                         │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Rust backend                                             │   │
│  │   api/        — GW2 client (reqwest + rate limiter) +   │   │
│  │                 DPAPI key storage + endpoint wrappers   │   │
│  │   db/         — SQLite schema/migrations + repository   │   │
│  │   sync/       — engine, achievements, progress, wv,     │   │
│  │                 items (definitions), inventory (account)│   │
│  │   timers/     — boss / meta schedule + spawn math       │   │
│  │   scorer/     — weighted urgency ranking                │   │
│  │   catalog/    — loaders for legendary + boss-link JSON  │   │
│  │   builds.rs   — static builds catalog loader            │   │
│  │   commands.rs — every #[tauri::command] handler         │   │
│  │   error.rs    — AppError enum (serializable to JS)      │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                       │
                       ▼
                GW2 official API (no third-party services)
```

## Crate / module layout

```
src-tauri/
├── Cargo.toml             — Rust deps (Tauri 2, rusqlite, reqwest, etc.)
├── tauri.conf.json        — three window declarations, capabilities
├── capabilities/
│   └── default.json       — permissions for all three windows
├── data/
│   ├── boss_schedule.json — 13 world bosses + 23 meta events + ley-line
│   ├── achievement_boss_links.json — achievement_id → boss_id metadata
│   ├── legendary_collections.json  — 32 curated legendary collections
│   └── builds.json        — curated meta builds catalog
└── src/
    ├── lib.rs             — Tauri Builder + setup hook + state mgmt
    ├── main.rs            — entry point (delegates to lib::run)
    ├── error.rs           — AppError + Result alias
    ├── api/
    │   ├── auth.rs        — ApiKey newtype + DPAPI encrypt/decrypt
    │   ├── client.rs      — reqwest client + token-bucket rate limiter
    │   └── endpoints.rs   — typed wrappers for /v2/* endpoints
    │                       (now includes bank, materials, characters,
    │                        items batch — items fetched in lang=fr)
    ├── builds.rs          — static builds catalog loader (include_str!)
    ├── catalog/
    │   └── mod.rs         — JSON loaders for legendary + boss-link
    ├── commands.rs        — all #[tauri::command] IPC handlers
    ├── db/
    │   ├── schema.rs      — 6 versioned migrations + migrate()
    │   └── repository.rs  — Db struct (Mutex<Connection>) + helpers
    ├── scorer/
    │   └── ranking.rs     — Weights + Scoreable + rank()
    ├── sync/
    │   ├── engine.rs      — SyncEngine (5 spawned loops)
    │   ├── achievements.rs— bulk paginated definition sync
    │   ├── progress.rs    — /v2/account/achievements + diff snapshot
    │   ├── wizardsvault.rs— WV daily/weekly/special
    │   ├── items.rs       — items_cache fetch + lookup (lang=fr)
    │   └── inventory.rs   — account_items (bank/materials/chars)
    └── timers/
        ├── schedule.rs    — WorldBoss / MetaEvent / Schedule
        └── engine.rs      — prev_spawn / next_spawn / current_meta_phase
scripts/
└── update_meta_events.py  — one-shot helper to bulk-replace meta_events

src/
├── App.tsx                — routes by window label
├── main.tsx               — ReactDOM root
├── components/
│   ├── Overlay.tsx        — main window shell (header, tabs, footer)
│   ├── BossesWindow.tsx   — bosses window root
│   ├── AchievementsWindow.tsx — achievements window root
│   ├── ApiKeySetup.tsx
│   ├── PinnedPanel.tsx    — exports BossesView + AchievementsView
│   ├── EventsTab.tsx
│   ├── CatalogView.tsx
│   ├── SearchView.tsx
│   ├── MyItemsView.tsx    — account-wide item search
│   ├── TodosView.tsx      — daily/weekly todos
│   ├── BuildsView.tsx     — builds manager
│   ├── WizardsVaultPanel.tsx
│   └── SettingsPanel.tsx
├── hooks/
│   ├── useHotkeys.ts      — Ctrl+Shift+G / H / B / P (global)
│   ├── useCrossWindowSync.ts — listens for pinned_changed +
│   │                            appearance_changed events
│   └── useCollapse.ts     — collapse window to header strip via setSize
├── lib/
│   ├── tauri.ts           — typed wrappers around invoke('cmd_…')
│   └── format.ts          — stripGw2Markup, fillRequirement,
│                            eventTimeLabel, wikiUrl
├── store/
│   ├── app.ts             — primary Zustand store (data + actions)
│   └── settings.ts        — appearance store (writes CSS variables)
├── types/
│   └── gw2.ts             — TS mirrors of every Rust IPC payload
└── styles/
    └── tailwind.css       — base + CSS variables + custom scrollbar
                            + .ui-zoom class for font-scale slider
```

## Data flow walk-through

When the user clicks "+ Pin" on an achievement in the Catalog tab:

1. React `onClick` → `useAppStore.pin(id, collectionKey)`
2. `pin()` calls `invoke("cmd_pin_achievement", { achievementId, collectionKey })`
3. Backend `commands.rs::cmd_pin_achievement` runs
   `db.pin_achievement(...)` → SQLite `INSERT OR IGNORE INTO pinned_achievements`
4. **Backend emits the `pinned_changed` Tauri event**
5. Frontend returns `Ok(())`. The originating window's `pin()` calls
   `refresh()` immediately (no need to wait for the event).
6. **Every other window**'s `useCrossWindowSync` listener fires and
   calls its own `refresh()` so the Pinned views in the bosses /
   achievements windows reflect the new pin within the same frame.
7. The originating window then calls `api.warmItemCache()` so any
   Item-typed bits referenced by the new pin get fetched from
   `/v2/items?lang=fr` and cached. If items were fetched, calls
   `refresh()` again to surface their names.

## State storage

One persistent store: SQLite at
`%APPDATA%\com.tripleseptconsulting.gw2overlay\gw2-overlay.sqlite`.

Tables (current schema version 6):

| Table | Origin | Purpose |
|---|---|---|
| `_migrations` | bootstrap | applied versions |
| `achievements` | bulk sync | full definition cache (~8 200 rows) |
| `account_progress` | progress sync | per-achievement current/max/done/bits |
| `daily_assignments` | (unused) | reserved for legacy `/v2/achievements/daily` (broken since WV) |
| `wizardsvault` | WV sync | (period_type, period_start, objective_id) rows |
| `settings` | manual | API key blob, appearance JSON, notification lead, last-sync timestamps |
| `achievement_metadata` | catalog load | `associated_boss`, tags, etc. |
| `pinned_achievements` | user | user's pinned achievement ids |
| `pinned_bosses` | user | user's pinned world boss / meta ids |
| `legendary_collections` | catalog load | curated catalog |
| `legendary_collection_members` | catalog load | (collection_key, achievement_id, step) |
| `items_cache` | items sync | id → French name + description |
| `account_items` | inventory sync | (item_id, location, location_detail, count) across bank / materials / shared / characters / equipped slots |
| `todos` | user | daily / weekly todos with auto-reset |

In-RAM stores (rebuilt at boot from SQLite):
- **Progress snapshot** in `SyncEngine` — `HashMap<u32, AccountAchievement>` used to diff progress changes.
- **Notified set** in `SyncEngine` — `HashSet<(boss_id, spawn_time)>` for de-duping toast notifications.

## SyncEngine lifecycle

`SyncEngine` is built in the Tauri `setup` hook **iff** a key is
stored, and is recreated by `cmd_set_api_key` on key rotation. It spawns
**five** background tokio tasks via `tauri::async_runtime::spawn`:

1. **Achievements bootstrap** — one-shot. Skips if
   `achievements_last_full_sync` is < 7 days ago. Else pulls
   `/v2/achievements?page=N&page_size=200` until exhausted.
2. **Progress loop** — `interval(300s)`. Pulls
   `/v2/account/achievements`, diffs against snapshot, persists.
3. **Wizard's Vault loop** — `interval(900s)`. Daily / weekly / special
   in sequence.
4. **Inventory loop** — `interval(1800s)`. Pulls bank + materials +
   shared inventory + every character's bags AND equipment, wipes
   and re-inserts `account_items`. Also warms `items_cache` (lang=fr).
5. **Boss watcher** — `interval(30s)`. For each pinned boss / meta:
   resolves via `world_bosses` or `meta_events` schedule arrays,
   computes time-to-spawn, fires a Windows toast if ≤ configured
   lead-time minutes. De-dupes per (id, spawn_time).

All loops listen on a `CancellationToken` so a key rotation triggers a
clean exit on the next tick.

## Cross-window event protocol

Each Tauri window runs in its own JS context with its own Zustand
store + DOM. Mutations in one window must broadcast for the others
to see them:

- `pinned_changed` — emitted by `cmd_pin_achievement /
  cmd_unpin_achievement / cmd_pin_boss / cmd_unpin_boss /
  cmd_remove_boss_group`. Listener calls `useAppStore.refresh()`.
- `appearance_changed` — emitted by `cmd_set_appearance`. Listener
  calls `useSettingsStore.load()` which re-applies CSS variables to
  the local document.

Wired by `src/hooks/useCrossWindowSync.ts`, called on mount in each
of the three top-level window components.

## Three-window architecture

Declared in `tauri.conf.json` with three `windows[]` entries (labels
`main`, `bosses`, `achievements`), each loading `index.html` with a
URL hash. `App.tsx` reads `getCurrentWindow().label` and renders the
matching root component.

Secondary windows (`bosses`, `achievements`) have a Tauri
`on_window_event` interceptor in `lib.rs`: their close event is
swallowed and replaced with `.hide()`, so the user can dismiss them
without quitting the app. Only the main window quits the app on close
(the ⏻ button in the main header calls
`cmd_save_state_and_quit` for a clean save + exit).

Each window:
- Calls `useCollapse()` to bind the header's ▴ / ▾ button to
  `getCurrentWindow().setSize`, shrinking to a 32 px header strip.
- Calls `useCrossWindowSync()` to subscribe to backend events.
- Wraps its scrollable content in a `<div class="ui-zoom">` whose
  `zoom` is bound to `--ui-scale` (set by Settings panel slider).
  Header buttons stay at 1× so they never get pushed off-screen.

## Window-state plugin (persistence)

`tauri-plugin-window-state` 2.4 saves position + size on window close,
but does NOT auto-restore. We trigger restore explicitly in `setup`:

```rust
for label in ["main", "bosses", "achievements"] {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.restore_state(StateFlags::all());
    }
}
```

`tauri.conf.json` declares `width`/`height` but **NOT** `x`/`y` — those
would override the plugin's restore. First-ever launch defaults to OS
positioning.

⚠ Killing the dev shell with Ctrl+C SIGINTs the process; window close
events never fire and saved positions are lost for that session. The
⏻ button in the main header calls `cmd_save_state_and_quit` which
explicitly invokes `app.save_window_state(StateFlags::all())` before
`app.exit(0)`.

## Rate limiting

`api::client::ApiClient` uses a token-bucket: capacity 300, refill 5/s
(= 300/min, 50 % below the documented 600/min limit). On 429 / 5xx,
exponential backoff (1s, 2s, 4s, 8s) up to 4 retries, then surfaces
`AppError::RateLimited` or `AppError::Unavailable(status)`. The robust
`cmd_check_api_key` treats *any* validation error as a soft failure
so a transient API hiccup doesn't kick the user back to ApiKeySetup;
real auth failures eventually surface from the periodic sync loops.

## Why these tech choices

- **Tauri 2 vs Electron** — ~20 MB binary, native transparent
  always-on-top window, lower CPU at rest.
- **SQLite (rusqlite bundled)** — atomic transactions, no DLL, fast
  indexed lookups for the 8 200-row achievements + variable-size
  account_items.
- **Zustand** vs Redux — minimal boilerplate. Per-window stores by
  necessity (separate JS contexts).
- **Tailwind + CSS variables** — Settings panel can patch
  `--accent-color`, `--bg-color-rgba`, `--ui-scale` at runtime without
  React re-renders.
- **DPAPI** vs Stronghold — Windows-native, no master password, no
  extra crate. Cross-platform was never a goal.
- **CSS `zoom`** vs `font-size` — the UI uses explicit Tailwind pixel
  sizes (`text-[10px]`, `text-xs`) so a font-size change wouldn't
  scale anything visible. `zoom: var(--ui-scale)` on the content div
  uniformly scales everything inside without breaking the header
  layout.
- **CSS variables for theme colors** — let one window's slider change
  the value, and the `appearance_changed` event triggers every other
  window's `loadSettings()` which re-applies those variables.

## Known constraints / non-goals

- Windows-only by design (DPAPI, WebView2, no Linux/macOS Tauri build
  pipeline).
- Requires GW2 in **windowed fullscreen** (exclusive fullscreen
  renders the overlay invisible — undetected today).
- One API key at a time. No multi-account.
- No system-tray icon. Closing the main window quits.
- Builds catalog ships with placeholder chat codes — real codes must
  be filled by the curator (see [README.md](../README.md) "Add a build
  to the Builds tab").
