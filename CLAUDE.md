# CLAUDE.md — agent instructions

Read at the start of every Claude Code session in this repo. Keep it
terse — it's loaded into every context.

## Quick orientation

- **What this is:** A Tauri 2 + React desktop overlay for Guild Wars 2.
  See [README.md](README.md) for the user-facing intro and
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical layout.
- **Current version:** **v0.1.11** (full English display). The
  v0.1.10 dual-fetch / `name_en` bilingual machinery was removed;
  every GW2 data string is fetched and displayed in English now.
- **Three windows** — `main`, `bosses`, `achievements`. Each has its
  own JS context and Zustand store; cross-window state goes via Tauri
  events (`pinned_changed`, `appearance_changed`, `hotkeys_changed`).
  Read the Cross-Window Event Protocol section in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before adding mutating
  commands.
- **Pivot history matters.** Five product/architecture pivots in
  [docs/PIVOTS.md](docs/PIVOTS.md). Read before any structural change.
- **What's planned:** [docs/ROADMAP.md](docs/ROADMAP.md). Most
  shipped; remaining backlog is mostly polish + aspirational features.
- **Bigger features go through a spec → plan workflow:**
  `docs/superpowers/specs/` then `docs/superpowers/plans/`. The
  v0.1.11 full-EN rollout used this and the docs are good reference
  for the next round.

## House rules

- **Conventional commits** — `feat(scope):`, `fix(scope):`,
  `chore(scope):`, `data(scope):`, `docs(scope):`, `refactor(scope):`,
  `diag(scope):`. Common scopes: `api`, `db`, `sync`, `timer`,
  `scorer`, `ui`, `pinned`, `items`, `events`, `notify`, `window`,
  `state`, `todos`, `builds`, `hotkeys`, `legendary`, `updater`,
  `release`, `release-notes`.
- **Git identity not set globally** — use
  `git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit ...`
  or commits will be unsigned and the user prompt will reject.
- **clippy warnings-as-errors** — `cargo clippy --manifest-path
  src-tauri/Cargo.toml --all-targets -- -D warnings` before any
  commit. TS strict mode also enforced via `tsconfig.json`.
- **Unit tests in same file as code; integration tests in
  `src-tauri/tests/`**. Integration tests hit the live GW2 API and
  are `#[ignore]`d by default.
- **No localStorage / sessionStorage from React.** All persistent
  state in SQLite via Tauri commands. Each window has its own
  Zustand store; reach for `useCrossWindowSync` if a state change
  needs to propagate.
- **Don't log the API key.** Ever. `ApiKey` doesn't implement Debug
  or Display on purpose. There's a project memory note about the
  broken `/v2/achievements/daily` endpoint — see that file.
- **Mutating commands must emit a Tauri event** if other windows
  need to see the change. Conventions:
  - `pinned_changed` after any pin / unpin / boss-remove / reset DB
    (handled in `commands.rs::emit_pinned_changed`).
  - `appearance_changed` after `cmd_set_appearance`.
  - `hotkeys_changed` after `cmd_set_hotkeys`.
  - Other domain events as you add them — name them in past tense.
- **`cmd_check_api_key` is intentionally tolerant.** Any failure
  short of an explicit "no key in DB" returns
  `Ok(Some(unvalidated_status))` so a transient API blip doesn't
  kick the user back to the setup screen. Real auth failures are
  caught by the periodic sync engine instead.
- **English everywhere** — every `/v2/*` endpoint that supports
  `lang` passes `&lang=en`. The user runs GW2 in FR but accepted the
  EN-only tradeoff so wiki links + community searches work. Don't
  reintroduce FR without a deliberate revert of Pivot 5.
- **`is_stale` checks both timestamp AND count=0.** When you add
  another bulk-sync-with-staleness pattern (e.g. periodic resync of
  a long-lived cache), mirror this — `settings`-based timestamps
  survive table wipes.

## Common tasks (with the right entry point)

| Task | Where to start |
|---|---|
| Add a new Tauri command | `src-tauri/src/commands.rs` + register in `lib.rs::generate_handler!` + typed wrapper in `src/lib/tauri.ts`. If mutates state visible across windows, also emit a Tauri event. |
| Add a SQLite table or column | Append migration block to `MIGRATIONS` in `src-tauri/src/db/schema.rs` (**never edit a past migration**). SQLite 3.43+ (rusqlite bundled) supports `ALTER TABLE DROP COLUMN`. Update `fresh_migration_creates_all_tables` test. |
| Add a new legendary | Append to `src-tauri/data/legendary_collections.json`. Verify ids against `https://api.guildwars2.com/v2/achievements/categories?ids=114,118,125,173`. |
| Add a leaf to a legendary recipe | Append to `src-tauri/data/legendary_recipes.json` (format 1.3). Verify item ids against `/v2/items?ids=...&lang=en`. Components are reusable; per-legendary `leaves` are inlined. |
| Link an achievement to a boss | Append to `src-tauri/data/achievement_boss_links.json`. Boss id must match `boss_schedule.json`. |
| Add a world boss or meta | Append to `src-tauri/data/boss_schedule.json`. `scripts/update_meta_events.py` is the bulk-replacement helper. |
| Add a build | Append to `src-tauri/data/builds.json`. Real chat codes come from in-game Hero panel → Build template → right-click → Copy. |
| Remap a hotkey at runtime | Settings → Hotkeys (live UI). To change the default, edit `src/hooks/useHotkeys.ts::HOTKEY_DEFAULTS` + the equivalent constants in `src-tauri/src/commands.rs`. |
| Add a hotkey action | Add the const + `register` call in `useHotkeys.ts::bind`; expose in `HotkeyConfig` (both Rust + TS); add a `HotkeyCapture` row in `SettingsPanel.tsx`. |
| Add an appearance setting | Extend `AppearanceSettings` in `commands.rs` + matching TS type in `src/types/gw2.ts` + slider in `src/components/SettingsPanel.tsx`. The cross-window `appearance_changed` event fires automatically. |
| Add a tab to the main window | `TABS` array in `Overlay.tsx` + new ViewKey value in `store/app.ts` + new component + render branch in the `view === ...` chain. |
| Debug a sync issue | Settings → Diagnostics → 📂 Open logs folder OR `$env:RUST_LOG="info,gw2_overlay_lib=debug"; npm run tauri dev`. |
| FE diagnostic in production | Use `api.logEvent("info", "MyComponent", "message")` — writes to the same file log as the backend (`console.log` is invisible in WebView2 release). |
| Add periodic work | Add a `spawn_*_loop` method to `SyncEngine` returning `JoinHandle<()>`, push it into `start()`. Uses `tokio::time::interval` + `CancellationToken`. |
| Ship a release | Pre-commit ritual passes → bump version in 3 files (`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`) → commit → tag `vX.Y.Z` → push tag. GH Actions does the rest (~15 min for MSI + NSIS + signed `latest.json`). |
| Plan a non-trivial feature | Use the brainstorming → spec → plan workflow. Specs at `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`, plans at `docs/superpowers/plans/`. v0.1.11 full-EN is the reference. |

## Pre-commit ritual

```powershell
# In src-tauri/
cargo test --lib                                       # 67 unit tests
cargo clippy --all-targets -- -D warnings              # zero warnings

# In repo root
npm run build                                          # tsc strict + Vite
```

If any fail, fix before committing. Integration tests
(`cargo test --test api_integration -- --ignored`) require a real
`GW2_API_KEY` env var and are not part of the pre-commit ritual.

CI (`.github/workflows/ci.yml`) runs the same gates on every push to
main + every PR.

## Things to NOT do without a strong reason

- Don't replace `rusqlite` with `sqlx`. The bundled feature gives a
  zero-DLL Windows build; switching would mean compile-time SQL
  schema discovery that doesn't fit a single-user app.
- Don't move from Zustand to Redux/Jotai. Same shape, more
  boilerplate.
- Don't add `localStorage` or browser-storage features behind React.
- Don't widen the IPC surface unnecessarily — each new `cmd_*` is a
  permission surface, a TypeScript binding to maintain, and a
  potential boot-race victim.
- Don't edit a past migration. Append a new one.
- Don't fetch from the GW2 API outside `api/client.rs`. The
  rate-limiter only works if everyone goes through the shared
  `ApiClient`.
- Don't pass `lang=fr` to any endpoint — Pivot 5 explicitly removed
  this. If you find yourself wanting FR names back, talk to the user
  first about a Settings toggle (see ROADMAP aspirational).
- Don't declare `x` / `y` in `tauri.conf.json` window blocks — they
  override the window-state plugin's restore.
- Don't use `tokio::spawn` from the setup hook — it panics. Use
  `tauri::async_runtime::spawn`.
- Don't hold the engine `Mutex` across an await (it's
  `std::sync::Mutex`, not `tokio::sync::Mutex`). Grab the
  `Arc<SyncEngine>` you need, drop the guard, then await.
- Don't bypass the boot-race retry in `src/lib/tauri.ts::invoke`.
  Multi-window Tauri 2 races webviews against `app.manage()`; every
  cmd must go through the retry-wrapped invoke.
- Don't subtract from `Instant::now()` in tests — fresh-boot Windows
  CI runners' monotonic clocks haven't run long enough and you'll
  panic. Synthesise time forward from a fixed `t0`.
- Don't set `bundle.createUpdaterArtifacts` to anything other than
  `true` if signing matters. Without it the `.sig` files won't be
  emitted and the updater client will reject the bundle.

## Useful one-liners for live debugging

```powershell
# Run dev with verbose Rust logs
$env:RUST_LOG = "info,gw2_overlay_lib=trace,gw2_overlay_lib::api::auth=trace"
npm run tauri dev

# Reset the SQLite DB if a migration goes sideways during dev
Remove-Item "$env:APPDATA\com.tripleseptconsulting.gw2overlay\gw2-overlay.sqlite*"

# Reset window positions if the plugin's state file gets weird
Remove-Item "$env:APPDATA\com.tripleseptconsulting.gw2overlay\.window-state"

# Open the user-facing log folder (same path the Diagnostics button uses)
explorer "$env:APPDATA\com.tripleseptconsulting.gw2overlay\logs\"

# Quick API probe
$env:GW2_API_KEY = "..."
Invoke-RestMethod -Uri "https://api.guildwars2.com/v2/tokeninfo" -Headers @{
  Authorization = "Bearer $env:GW2_API_KEY"
  "User-Agent"  = "gw2-overlay-debug"
}
```

## Release / hotfix workflow

```powershell
# Bump versions in 3 files:
# - package.json
# - src-tauri/Cargo.toml
# - src-tauri/tauri.conf.json

# Pre-commit ritual (see above)

git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" `
    commit -am "chore(release): bump to X.Y.Z"
git push origin main

git tag -a vX.Y.Z -m "vX.Y.Z — short summary"
git push origin vX.Y.Z
# → GitHub Actions release.yml fires, ~15 min for MSI + NSIS + latest.json
```

The `TAURI_SIGNING_PRIVATE_KEY` GH secret signs the bundles via
minisign. The corresponding pubkey is hard-coded in `tauri.conf.json`.
If the secret rotates, the user must regenerate both — losing the
private key means losing the ability to push updates to existing
installs.

## Where things go wrong (recent gotchas)

- **API key seems to "disappear"** — almost always Pivot 5's
  boot-race. Verify `lib/tauri.ts::invoke` retry wrapper is still in
  place. Add `cmd_log_event` traces around `store.checkApiKey` if
  the symptom is fresh.
- **Updater install fails with 404** — the asset URL in `latest.json`
  doesn't replace spaces with dots. Check `release.yml` for the
  `$urlName = $exe.Name -replace ' ', '.'` line.
- **NSIS bundle missing `.sig`** — either `bundle.createUpdaterArtifacts`
  is missing/false in `tauri.conf.json`, or the
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env var is non-empty when the
  key was generated without one. The workflow hardcodes it to `""`.
- **Boss disappears mid-fight** — the boss watcher's `prev_spawn`
  fallback handles in-progress events; if you change `current_meta_
  phase` semantics, run the entire `timers::engine::tests` to catch
  regressions.
- **"Item #N" / "Skin #N" displayed** — items/skins cache hasn't been
  warmed for that id yet. `cmd_warm_item_cache` fetches missing ids
  referenced by *pinned* achievements. Non-pinned references (e.g.
  legendary recipe leaves shown in Catalog) aren't warmed yet — see
  ROADMAP backlog.
- **Boss watcher fires twice for the same spawn** — check the
  notified-set retention. Currently `chrono::Duration::hours(1)`
  retention keeps recent entries.
