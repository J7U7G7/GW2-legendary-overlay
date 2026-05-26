# Architecture

Technical deep-dive. For a user-facing intro see [README.md](../README.md).
For the history of how we got here see [PIVOTS.md](PIVOTS.md).

Current version: **v0.1.11** (full English display).

## Top-down view

```
┌─────────────────────────────────────────────────────────────────┐
│ Tauri 2 process                                                 │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Three independent WebView2 windows                       │   │
│  │   each loads index.html with a different URL hash:       │   │
│  │     - #main         → <Overlay/> (config + 7 tabs)       │   │
│  │     - #bosses       → <BossesWindow/> (pinned bosses)    │   │
│  │     - #achievements → <AchievementsWindow/> (pinned ach) │   │
│  │   App.tsx routes by getCurrentWindow().label.            │   │
│  │   Each window has its OWN JS context + Zustand store —   │   │
│  │   cross-window sync goes through Tauri events.           │   │
│  └──────────────────────────────────────────────────────────┘   │
│                       ▲                                         │
│                       │ tauri::invoke (typed via lib/tauri.ts)  │
│                       │ ↳ retry wrapper around 'state not       │
│                       │   managed' boot race                    │
│                       ▼                                         │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Rust backend                                             │   │
│  │   api/        — GW2 client (reqwest + rate limiter) +   │   │
│  │                 DPAPI key storage + endpoint wrappers   │   │
│  │   db/         — SQLite schema/migrations (v10) + Db    │   │
│  │   sync/       — engine, achievements, progress, wv,     │   │
│  │                 inventory, items, skins, wallet         │   │
│  │   timers/     — boss / meta schedule + spawn math       │   │
│  │   scorer/     — weighted urgency ranking                │   │
│  │   catalog/    — loaders for legendary + boss-link JSON  │   │
│  │   legendary.rs — recipe walker + progress aggregator    │   │
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
├── Cargo.toml                  — Rust deps (Tauri 2, rusqlite, reqwest,
│                                 tauri-plugin-{updater,process},
│                                 tracing-appender, windows-sys)
├── tauri.conf.json             — 3 window declarations, plugins.updater
│                                 with pubkey, bundle.createUpdaterArtifacts,
│                                 NSIS installMode=passive
├── capabilities/
│   └── default.json            — permissions for all 3 windows incl.
│                                 updater + process + opener path
├── data/
│   ├── boss_schedule.json      — 13 world bosses + 23 meta events + ley-line
│   ├── achievement_boss_links.json — achievement_id → boss_id metadata
│   ├── legendary_collections.json  — 32 curated legendary collections
│   ├── legendary_recipes.json  — components + per-legendary leaves
│   │                              (Smart Legendary tier 2 curated, format 1.3)
│   └── builds.json             — curated meta builds catalog (Snowcrows +
│                                  MetaBattle, chat codes still placeholders)
└── src/
    ├── lib.rs                  — Tauri Builder + setup hook + state mgmt +
    │                              file-log layer + panic hook
    ├── main.rs                 — entry point (delegates to lib::run)
    ├── error.rs                — AppError + Result alias
    ├── api/
    │   ├── auth.rs             — ApiKey newtype + DPAPI encrypt/decrypt
    │   │                          + tagged per-step load_api_key logging
    │   ├── client.rs           — reqwest client + token-bucket rate limiter
    │   └── endpoints.rs        — typed wrappers for /v2/*. All localized
    │                              endpoints fetch lang=en since v0.1.11.
    ├── builds.rs               — static builds catalog loader (include_str!)
    ├── catalog/
    │   └── mod.rs              — JSON loaders for legendary + boss-link
    ├── commands.rs             — all #[tauri::command] IPC handlers
    │                              + emit_pinned_changed /
    │                                emit_appearance_changed helpers
    │                              + cmd_log_event (FE → tracing bridge)
    ├── db/
    │   ├── schema.rs           — 10 versioned migrations + migrate()
    │   └── repository.rs       — Db struct (Mutex<Connection>) +
    │                              wipe_data() + get_setting/set_setting
    ├── legendary.rs            — recipe walker: aggregate_needs(),
    │                              compute_progress(), CachedItem-aware
    │                              missing-leaves ranking
    ├── scorer/
    │   └── ranking.rs          — Weights + Scoreable + rank()
    ├── sync/
    │   ├── engine.rs           — SyncEngine (6 spawned loops)
    │   ├── achievements.rs     — bulk paginated definition sync;
    │   │                          is_stale() also checks count(*)==0
    │   ├── progress.rs         — /v2/account/achievements + diff snapshot
    │   ├── wizardsvault.rs     — WV daily/weekly/special (special is
    │   │                          soft-fail since the endpoint frequently
    │   │                          returns a non-period shape)
    │   ├── items.rs            — items_cache fetch + lookup (lang=en)
    │   ├── skins.rs            — skins_cache fetch + lookup (lang=en)
    │   ├── inventory.rs        — account_items (bank/materials/chars/eq.)
    │   └── wallet.rs           — currencies + account_currencies sync
    └── timers/
        ├── schedule.rs         — WorldBoss / MetaEvent / Schedule
        └── engine.rs           — prev_spawn / next_spawn /
                                   current_meta_phase (with filler
                                   filtering: Idle/Reset/Prep skipped)
scripts/
└── update_meta_events.py       — one-shot helper for bulk meta-events edit

src/
├── App.tsx                     — routes by window label
├── main.tsx                    — ReactDOM root
├── components/
│   ├── Overlay.tsx             — main window shell (header, tabs, footer,
│   │                              version display)
│   ├── BossesWindow.tsx        — bosses window root
│   ├── AchievementsWindow.tsx  — achievements window root
│   ├── ApiKeySetup.tsx
│   ├── PinnedPanel.tsx         — exports BossesView + AchievementsView,
│   │                              wikiUrlForBit (EN deep-link cascade)
│   ├── EventsTab.tsx
│   ├── CatalogView.tsx         — Smart Legendary 📦 N% recipe progress
│   ├── SearchView.tsx
│   ├── MyItemsView.tsx         — account-wide item + currency search
│   ├── TodosView.tsx           — daily/weekly todos
│   ├── BuildsView.tsx          — builds manager with filter chips
│   ├── WizardsVaultPanel.tsx
│   ├── UpdatePrompt.tsx        — non-modal auto-update banner
│   └── SettingsPanel.tsx       — appearance + hotkeys + diagnostics
│                                 + danger zone (reset window, reset DB)
├── hooks/
│   ├── useHotkeys.ts           — global shortcuts (configurable since
│   │                              v0.1.2). Robust per-shortcut fallback;
│   │                              listens for hotkeys_changed event.
│   ├── useCrossWindowSync.ts   — listens for pinned_changed +
│   │                              appearance_changed
│   └── useCollapse.ts          — collapse window to header strip via setSize
├── lib/
│   ├── tauri.ts                — typed invoke wrappers + boot-race
│   │                              retry around 'state not managed'
│   └── format.ts               — stripGw2Markup, fillRequirement,
│                                  eventTimeLabel, wikiUrl
├── store/
│   ├── app.ts                  — primary Zustand store (data + actions,
│   │                              apiKeyChecked flag)
│   └── settings.ts             — appearance + hotkeys store (writes CSS
│                                  variables, broadcasts to backend)
├── types/
│   └── gw2.ts                  — TS mirrors of every Rust IPC payload
└── styles/
    └── tailwind.css            — base + CSS variables + custom scrollbar
                                  + .ui-zoom class for font-scale slider

.github/
├── workflows/
│   ├── ci.yml                  — push/PR → cargo test + clippy + npm build
│   └── release.yml             — tag v* → tauri build + sign + manifest +
│                                  upload to GH Release
└── ISSUE_TEMPLATE/
    ├── bug_report.yml
    ├── feature_request.yml
    └── config.yml              — disables blank issues + contact links

docs/
├── ARCHITECTURE.md             — this file
├── PIVOTS.md                   — 5 pivots + smaller-findings list
├── ROADMAP.md                  — shipped / scoped / aspirational
└── superpowers/
    ├── specs/                  — design docs (brainstorming output)
    └── plans/                  — bite-sized implementation plans
                                  (the v0.1.11 full-EN rollout is the
                                  reference for this workflow)
```

## Data flow walk-through

When the user clicks "+ Pin" on an achievement in the Catalog tab:

1. React `onClick` → `useAppStore.pin(id, collectionKey)`.
2. `pin()` calls `invoke("cmd_pin_achievement", { achievementId, collectionKey })`
   via the retry-wrapped invoke in `lib/tauri.ts`.
3. Backend `commands.rs::cmd_pin_achievement` runs
   `db.pin_achievement(...)` → SQLite `INSERT OR IGNORE INTO
   pinned_achievements`.
4. **Backend emits the `pinned_changed` Tauri event.**
5. Frontend returns `Ok(())`. The originating window's `pin()` calls
   `refresh()` immediately (no need to wait for the event).
6. **Every other window**'s `useCrossWindowSync` listener fires and
   calls its own `refresh()` so the Pinned views in the bosses /
   achievements windows reflect the new pin within the same frame.
7. The originating window then calls `api.warmItemCache()` so any
   Item-typed or Skin-typed bits referenced by the new pin get fetched
   from `/v2/items?lang=en` + `/v2/skins?lang=en` and cached. If items
   were fetched, calls `refresh()` again to surface their names.

## State storage

One persistent store: SQLite at
`%APPDATA%\com.tripleseptconsulting.gw2overlay\gw2-overlay.sqlite`.

Tables (current schema version **10**):

| Table | Origin | Purpose |
|---|---|---|
| `_migrations` | bootstrap | applied versions |
| `achievements` | bulk sync | full definition cache (~8 200 rows, EN names) |
| `account_progress` | progress sync | per-achievement current/max/done/bits |
| `daily_assignments` | (zombie since pivot 1) | empty schema, kept because migrations are append-only |
| `wizardsvault` | WV sync | (period_type, period_start, objective_id) rows |
| `settings` | manual | API key blob, appearance JSON, hotkeys, notification lead, last-sync timestamps |
| `achievement_metadata` | catalog load | `associated_boss`, tags, etc. |
| `pinned_achievements` | user | user's pinned achievement ids |
| `pinned_bosses` | user | user's pinned world boss / meta ids |
| `legendary_collections` | catalog load | curated catalog (32 entries) |
| `legendary_collection_members` | catalog load | (collection_key, achievement_id, step) |
| `items_cache` | items sync | id → EN name + description (after v10 migration) |
| `skins_cache` | skins sync | id → EN name + description |
| `account_items` | inventory sync | (item_id, location, location_detail, count) across bank / materials / shared / characters / equipped slots |
| `todos` | user | daily / weekly todos with auto-reset |
| `currencies` | wallet sync | id → EN name + icon + sort_order |
| `account_currencies` | wallet sync | currency_id → value |

Migration history at a glance: v1 base spec → v2 pinning + legendary
catalog → v3 boss pinning → v4 items cache → v5 account items → v6
todos → v7 wallet → v8 skins → v9 added `name_en` columns → v10
**removed** the `name_en` columns + wiped FR-cached tables for the
full-English switch.

`legendary_recipes.json` (curated EN recipe data, not in SQLite) lives
in `src-tauri/data/` and is `include_str!`-embedded at compile time.
Updates require a rebuild.

In-RAM stores (rebuilt at boot):
- **Progress snapshot** in `SyncEngine` — `HashMap<u32,
  AccountAchievement>` used to diff progress changes.
- **Notified set** in `SyncEngine` — `HashSet<(boss_id, spawn_time)>`
  for de-duping toast notifications.

## SyncEngine lifecycle

`SyncEngine` is built in the Tauri `setup` hook **iff** a key is
stored, and is recreated by `cmd_set_api_key` on key rotation. It
spawns **six** background tokio tasks via `tauri::async_runtime::spawn`:

1. **Achievements bootstrap** — one-shot. `is_stale` returns true if
   the table is empty OR if the `last_full_sync` timestamp is older
   than 7 days. The empty-table check matters: schema v10 wipes the
   table but the timestamp survives in `settings` → without the
   count check the bulk re-sync would never trigger.
2. **Progress loop** — `interval(300s)`. Pulls
   `/v2/account/achievements`, diffs against snapshot, persists.
3. **Wallet loop** — `interval(300s)`. Pulls `/v2/account/wallet`,
   resolves any new currency definitions via `/v2/currencies?ids=...
   &lang=en`, upserts.
4. **Wizard's Vault loop** — `interval(900s)`. Daily / weekly /
   special in sequence. The "special" endpoint frequently returns a
   shape that fails to decode (no special event active) — that error
   is downgraded to a `debug!` line.
5. **Inventory loop** — `interval(1800s)`. Pulls bank + materials +
   shared inventory + every character's bags AND equipment, wipes and
   re-inserts `account_items`. Also warms `items_cache` (lang=en).
6. **Boss watcher** — `interval(30s)`. For each pinned boss / meta:
   resolves via `world_bosses` or `meta_events` schedule arrays,
   computes time-to-spawn, fires a Windows toast if ≤ configured
   lead-time minutes. De-dupes per (id, spawn_time).

All loops listen on a `CancellationToken` so a key rotation triggers
a clean exit on the next tick.

## Smart Legendary Selector

`src-tauri/src/legendary.rs` walks the curated recipe data and
produces per-legendary progress:

1. **Recipe model** (`legendary_recipes.json`, format 1.3):
   - `components` — named bundles of leaves (Gift of Fortune, Gift of
     Mastery, Mystic Tribute, Gen 1 Signature, Vision Crystal).
   - `legendaries` — per-`collection_key` entry referencing zero or
     more components plus zero or more direct leaves.
   - Each leaf: `{ kind: "item" | "currency", id, quantity, name,
     notes? }`.
2. **Walker** (`aggregate_needs`): flattens all referenced components
   + direct leaves into a `HashMap<(kind, id), needed>`.
3. **Compute progress** (`compute_progress`): cross-references aggregated
   needs against `account_items` (SUM by item_id) + `account_currencies`,
   computes ratio, returns top-N missing leaves sorted by absolute
   missing amount.
4. **IPC** (`cmd_legendary_progress`): returns
   `Vec<LegendaryProgress>` sorted by completion ratio descending.
5. **UI** (`CatalogView.tsx`): each card shows `📦 N%` + expandable
   "Recipe Progress" section with the missing leaves and a "READY?"
   banner at ratio ≥ 0.95.

Tier 2 (v0.1.10) added: Vision Crystal expanded to raw mats; LWS3/4
currencies for trinkets; Howl + Frostfang craftable precursor
sub-recipes; Gen 2 weapons multiply Vision Crystal ×4 via extra
direct leaves.

## Cross-window event protocol

Each Tauri window runs in its own JS context with its own Zustand
store + DOM. Mutations in one window must broadcast for the others
to see them:

- `pinned_changed` — emitted by `cmd_pin_achievement /
  cmd_unpin_achievement / cmd_pin_boss / cmd_unpin_boss /
  cmd_remove_boss_group / cmd_reset_database`. Listener calls
  `useAppStore.refresh()`.
- `appearance_changed` — emitted by `cmd_set_appearance`. Listener
  calls `useSettingsStore.load()` which re-applies CSS variables to
  the local document.
- `hotkeys_changed` — emitted by `cmd_set_hotkeys`. Listener (in
  `useHotkeys`) re-binds the global shortcuts to the new combos.

Wired by `src/hooks/useCrossWindowSync.ts` (pinned + appearance) and
`src/hooks/useHotkeys.ts` (hotkeys), called on mount in each window.

## Three-window architecture

Declared in `tauri.conf.json` with three `windows[]` entries (labels
`main`, `bosses`, `achievements`), each loading `index.html` with a
URL hash. `App.tsx` reads `getCurrentWindow().label` and renders the
matching root component.

Secondary windows (`bosses`, `achievements`) have a Tauri
`on_window_event` interceptor in `lib.rs`: their close event is
swallowed and replaced with `.hide()`, so the user can dismiss them
without quitting the app. Only the main window quits the app on
close (the ⏻ button in the main header calls `cmd_save_state_and_quit`
for a clean save + exit).

Each window:
- Calls `useCollapse()` to bind the header's ▴ / ▾ button to
  `getCurrentWindow().setSize`, shrinking to a 32 px header strip.
- Calls `useCrossWindowSync()` to subscribe to backend events.
- Wraps its scrollable content in a `<div class="ui-zoom">` whose
  `zoom` is bound to `--ui-scale` (set by Settings panel slider).
  Header buttons stay at 1× so they never get pushed off-screen.
- Calls `useAppStore.checkApiKey()` on mount. Gates UI on
  `apiKeyChecked` flag (true after the first check resolves, success
  OR error) to avoid the FR-era flash where ApiKeySetup briefly
  rendered while the check was in-flight.

Min sizes enforced in `tauri.conf.json` to prevent the layout meltdown
the user hit when shrinking the main window too small: main 320×220,
bosses + achievements 240×160.

## Boot-time IPC race

Multi-window Tauri apps surface a race: the WebView2 webviews start
loading in parallel with the setup hook. The main window's React
`useEffect` can fire `cmd_check_api_key` BEFORE `app.manage(AppState)`
has run — the IPC layer then throws `"state not managed for field
state on command cmd_check_api_key"`.

Fix: `src/lib/tauri.ts::invoke` wraps Tauri's raw invoke with a
6-attempt exponential-backoff retry loop (80ms × attempt) that only
fires on this specific error string. Other errors propagate
immediately. Every `api.*` function routes through this wrapper, so
the race is handled uniformly.

## Window-state plugin (persistence)

`tauri-plugin-window-state` 2.4 saves position + size on window
close, but does NOT auto-restore. We trigger restore explicitly in
`setup`:

```rust
for label in ["main", "bosses", "achievements"] {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.restore_state(StateFlags::all());
    }
}
```

`tauri.conf.json` declares `width`/`height` but **NOT** `x`/`y` —
those would override the plugin's restore. First-ever launch defaults
to OS positioning.

If a window ends up off-screen (the achievement-window-fullscreen
incident), Settings → Window layout → "Reset window layout & restart"
calls `cmd_reset_window_layout` which removes the `.window-state` file
+ triggers `relaunch()`.

⚠ Killing the dev shell with Ctrl+C SIGINTs the process; window close
events never fire and saved positions are lost for that session. The
⏻ button in the main header calls `cmd_save_state_and_quit` which
explicitly invokes `app.save_window_state(StateFlags::all())` before
`app.exit(0)`.

## Auto-updater

`tauri-plugin-updater` checks
`https://github.com/J7U7G7/GW2-legendary-overlay/releases/latest/download/latest.json`
on app start (only from the main window). When a newer version is
found, `UpdatePrompt.tsx` renders a non-modal banner showing both
`available v0.1.X` and `(you have v0.1.Y)` so version mismatches are
visible at a glance.

Click "Install now" → `downloadAndInstall` + `relaunch()`. The NSIS
installer runs in `installMode: "passive"` so the user sees progress
without UAC re-prompting.

Bundles are signed via **minisign**, not Authenticode. The public key
lives in `tauri.conf.json::plugins.updater.pubkey`; the private key is
the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret. `bundle.createUpdater
Artifacts: true` in `tauri.conf.json` is required or `tauri build`
silently produces unsigned bundles.

## Logging

Logging goes to TWO sinks: stdout (dev console) and a daily-rotated
file at `%APPDATA%/com.tripleseptconsulting.gw2overlay/logs/
gw2-overlay.log.YYYY-MM-DD`. Set up in `lib.rs::run()` BEFORE the
Tauri builder via `tracing_subscriber::registry()` with two `fmt`
layers, ANSI off for the file layer.

A custom `std::panic::set_hook` converts Rust panics (otherwise
silently swallowed for spawned tokio tasks) into `tracing::error!`
entries. The original hook is chained afterwards.

`cmd_log_event(level, target, message)` is a FE→backend bridge that
lets React write to the same file via `api.logEvent("info",
"Overlay.render", "…")`. Used wherever `console.log` would be
invisible in a production WebView2 build (no devtools). Currently
threaded through `store.checkApiKey` (state transitions) and
`Overlay.tsx` (render-branch decisions). Critical for diagnosing
production-only bugs.

## Rate limiting

`api::client::ApiClient` uses a token-bucket: capacity 300, refill
5/s (= 300/min, 50% below the documented 600/min limit). On 429 / 5xx,
exponential backoff (1s, 2s, 4s, 8s) up to 4 retries, then surfaces
`AppError::RateLimited` or `AppError::Unavailable(status)`. The robust
`cmd_check_api_key` treats *any* validation error as a soft failure
so a transient API hiccup doesn't kick the user back to ApiKeySetup;
real auth failures eventually surface from the periodic sync loops.

`Bucket` exposes a `try_take_at(now: Instant)` for tests so the test
suite never subtracts from `Instant::now()` (underflows on
fresh-boot Windows CI runners). Tests synthesise time forward only.

## CI / Release pipeline

Two GitHub Actions workflows:

- **`.github/workflows/ci.yml`** — push to `main` + PR runs cargo
  test --lib (67 tests) + cargo clippy --all-targets -D warnings +
  npm run build on windows-latest with Node 24 + Rust stable +
  Swatinem/rust-cache.
- **`.github/workflows/release.yml`** — tag push (`v*`) signs MSI +
  NSIS bundles (uses `TAURI_SIGNING_PRIVATE_KEY` repo secret +
  hardcoded empty password), generates `latest.json` with the
  asset URL (spaces replaced with dots per GitHub's asset-URL
  transformation), uploads installers + manifest to the GH Release.

A "Verify signing key secret is present" pre-step asserts the secret
is non-empty by printing its length — never the value.

## Why these tech choices

- **Tauri 2 vs Electron** — ~20 MB binary, native transparent
  always-on-top window, lower CPU at rest.
- **SQLite (rusqlite bundled)** — atomic transactions, no DLL, fast
  indexed lookups. Bundled feature gives SQLite 3.43+ so `ALTER TABLE
  DROP COLUMN` works in migrations.
- **Zustand** vs Redux — minimal boilerplate. Per-window stores by
  necessity (separate JS contexts).
- **Tailwind + CSS variables** — Settings panel can patch
  `--accent-color`, `--bg-color-rgba`, `--ui-scale` at runtime
  without React re-renders.
- **DPAPI** vs Stronghold — Windows-native, no master password, no
  extra crate. Cross-platform was never a goal.
- **CSS `zoom`** vs `font-size` — the UI uses explicit Tailwind pixel
  sizes (`text-[10px]`, `text-xs`) so a font-size change wouldn't
  scale anything visible. `zoom: var(--ui-scale)` on the content div
  uniformly scales everything inside without breaking the header
  layout.
- **English-only display** — accepted lost FR-search usability in
  exchange for consistency with the GW2 EN ecosystem (wiki,
  snowcrows, ArcDPS, BlishHUD). See Pivot 5 in
  [PIVOTS.md](PIVOTS.md).
- **Minisign for updater** vs Authenticode — minisign signs the
  bundle contents (the updater verifies signatures); Authenticode
  would sign the binary for SmartScreen. Both are independent; we
  use minisign because it's free and the updater plugin requires
  it. SmartScreen warnings are accepted for personal use.

## Known constraints / non-goals

- Windows-only by design (DPAPI, WebView2, no Linux/macOS Tauri
  build pipeline).
- Requires GW2 in **windowed fullscreen** (exclusive fullscreen
  renders the overlay invisible — undetected today).
- One API key at a time. No multi-account.
- No system-tray icon. Closing the main window quits.
- Builds catalog ships with placeholder chat codes — real codes
  must be pasted from in-game (Hero panel → Build template →
  right-click → Copy).
- English-only display since v0.1.11. A future Settings toggle could
  re-introduce FR but would require keeping both name columns and
  re-running bulk sync on toggle.
