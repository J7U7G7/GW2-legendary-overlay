# CLAUDE.md — agent instructions

Read at the start of every Claude Code session in this repo. Keep it
terse — it's loaded into every context.

## Quick orientation

- **What this is:** A Tauri 2 + React desktop overlay for Guild Wars 2.
  See [README.md](README.md) for the user-facing intro and
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical layout.
- **Three windows** — `main`, `bosses`, `achievements`. Each has its
  own JS context and Zustand store; cross-window state goes via Tauri
  events. Read the Cross-Window Event Protocol section in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before adding mutating
  commands.
- **Pivot history matters.** Four product/architecture pivots in
  [docs/PIVOTS.md](docs/PIVOTS.md). Read before any structural change.
- **What's planned:** [docs/ROADMAP.md](docs/ROADMAP.md). The Smart
  Legendary Selector recipe walker has a fully-designed plan there.

## House rules

- **Conventional commits** — `feat(scope):`, `fix(scope):`,
  `chore(scope):`, `data(scope):`, `docs(scope):`. Common scopes:
  `api`, `db`, `sync`, `timer`, `scorer`, `ui`, `pinned`, `items`,
  `events`, `notify`, `window`, `state`, `todos`, `builds`.
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
- **Mutating commands must emit a Tauri event** if other windows need
  to see the change. Conventions:
  - `pinned_changed` after any pin / unpin / boss-remove (handled in
    `commands.rs::emit_pinned_changed`).
  - `appearance_changed` after `cmd_set_appearance`.
  - Other domain events as you add them — name them in past tense.
- **`cmd_check_api_key` is intentionally tolerant.** Any failure
  short of an explicit "no key in DB" returns
  `Ok(Some(unvalidated_status))` so a transient API blip doesn't kick
  the user back to the setup screen. Real auth failures are caught by
  the periodic sync engine instead.

## Common tasks (with the right entry point)

| Task | Where to start |
|---|---|
| Add a new Tauri command | `src-tauri/src/commands.rs` + register in `lib.rs` + add typed wrapper in `src/lib/tauri.ts`. If it mutates state visible across windows, also emit a Tauri event. |
| Add a SQLite table | Append to `MIGRATIONS` in `src-tauri/src/db/schema.rs` (**never edit a past migration**). Update the `fresh_migration_creates_all_tables` test. |
| Add a new legendary | Append to `src-tauri/data/legendary_collections.json`. Verify ids against `https://api.guildwars2.com/v2/achievements/categories?ids=114,118,125,173`. |
| Link an achievement to a boss | Append to `src-tauri/data/achievement_boss_links.json`. Boss id must match `boss_schedule.json`. |
| Add a world boss or meta | Append to `src-tauri/data/boss_schedule.json`. `scripts/update_meta_events.py` shows the bulk-replacement pattern (used after the v5.2 wiki audit). |
| Add a build | Append to `src-tauri/data/builds.json`. Real chat codes come from in-game Hero panel → Build template → right-click → Copy. |
| Add a hotkey | `src/hooks/useHotkeys.ts`. Add the const + the `register` call inside `setup`. The hook also handles unregisterAll on cleanup. |
| Add an appearance setting | Extend `AppearanceSettings` in `commands.rs` + the matching TS type in `src/types/gw2.ts` + a slider in `src/components/SettingsPanel.tsx`. Don't forget the cross-window `appearance_changed` event will fire automatically. |
| Add a tab to the main window | `TABS` array in `Overlay.tsx` + a new ViewKey value + a new component + a render branch in the `view === ...` chain. |
| Debug a sync issue | `$env:RUST_LOG="info,gw2_overlay_lib=debug"` before `npm run tauri dev`. The sync engine + commands log liberally. |
| Add periodic work | Add a `spawn_*_loop` method to `SyncEngine` returning a `JoinHandle<()>`, push it into the `start()` vec. Uses `tokio::time::interval` + `CancellationToken`. |

## Pre-commit ritual

```powershell
# In src-tauri/
cargo test --lib                                        # 56+ unit tests
cargo clippy --all-targets -- -D warnings               # zero warnings

# In repo root
npm run build                                           # tsc strict + Vite
```

If any fail, fix before committing. Integration tests
(`cargo test --test api_integration -- --ignored`) require a real
`GW2_API_KEY` env var and are not part of the pre-commit ritual.

## Things to NOT do without a strong reason

- Don't replace `rusqlite` with `sqlx`. The bundled feature gives a
  zero-DLL Windows build; switching would mean compile-time SQL
  schema discovery that doesn't fit a single-user app.
- Don't move from Zustand to Redux/Jotai. Same shape, more
  boilerplate.
- Don't add `localStorage` or browser-storage features behind React.
- Don't widen the IPC surface unnecessarily — each new `cmd_*` is a
  permission surface and a TypeScript binding to maintain.
- Don't edit a past migration. Append a new one.
- Don't fetch from the GW2 API outside `api/client.rs`. The
  rate-limiter only works if everyone goes through the shared
  `ApiClient`.
- Don't declare `x` / `y` in `tauri.conf.json` window blocks — they
  override the window-state plugin's restore.
- Don't use `tokio::spawn` from the setup hook — it panics. Use
  `tauri::async_runtime::spawn`.
- Don't hold the engine `Mutex` across an await (it's `std::sync::
  Mutex`, not `tokio::sync::Mutex`). Grab the `Arc<SyncEngine>` you
  need, drop the guard, then await.

## Useful one-liners for live debugging

```powershell
# Run dev with verbose Rust logs
$env:RUST_LOG = "info,gw2_overlay_lib=trace,gw2_overlay_lib::api::auth=trace"
npm run tauri dev

# Reset the SQLite DB if a migration goes sideways during dev
Remove-Item "$env:APPDATA\com.tripleseptconsulting.gw2overlay\gw2-overlay.sqlite*"

# Reset window positions if the plugin's state file gets weird
Remove-Item "$env:APPDATA\com.tripleseptconsulting.gw2overlay\.window-state.json"

# Quick API probe
$env:GW2_API_KEY = "..."
Invoke-RestMethod -Uri "https://api.guildwars2.com/v2/tokeninfo" -Headers @{
  Authorization = "Bearer $env:GW2_API_KEY"
  "User-Agent"  = "gw2-overlay-debug"
}
```
