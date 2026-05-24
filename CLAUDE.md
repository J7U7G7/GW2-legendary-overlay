# CLAUDE.md — agent instructions

This file is read by Claude Code (and similar agents) at the start of
each session in this repository. Keep it terse — it's loaded into every
context.

## Quick orientation

- **What this is:** A Tauri 2 + React desktop overlay for Guild Wars 2.
  See [README.md](README.md) for the user-facing intro and
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical layout.
- **Where the work lives:** Rust backend in `src-tauri/src/`, React
  frontend in `src/`. Both are in this one repo (no submodules).
- **History matters:** Read [docs/PIVOTS.md](docs/PIVOTS.md) before
  making any structural change. The project changed shape twice
  during dev; understanding why prevents re-doing the wrong thing.
- **What's planned:** [docs/ROADMAP.md](docs/ROADMAP.md) lists the
  backlog. Pull from there before inventing new scope.

## House rules (from prior sessions)

- **Conventional commits** — `feat(scope):`, `fix(scope):`,
  `chore(scope):`, `data(scope):`. The scope is usually one of:
  `api`, `db`, `sync`, `timer`, `scorer`, `ui`, `pinned`, `items`,
  `events`, `notify`, `window`.
- **clippy warnings-as-errors** — Run `cargo clippy --manifest-path
  src-tauri/Cargo.toml --all-targets -- -D warnings` before any commit.
  TS strict mode is also enforced via `tsconfig.json`.
- **One test for one thing** — unit tests in the same file as the
  code; integration tests in `src-tauri/tests/`. The integration tests
  hit the live GW2 API and are `#[ignore]`d by default.
- **Don't write to localStorage / sessionStorage from React.** All
  persistent state lives in SQLite via Tauri commands. localStorage
  has bitten this project before (Vite HMR vs. zustand re-mounts).
- **Don't log the API key.** Ever. `ApiKey` doesn't implement Debug or
  Display on purpose. There's a project memory note in
  `~/.claude/projects/.../memory/` about the broken `/v2/achievements/
  daily` endpoint — that file is the source of truth on that quirk.
- **Multiple windows, multiple zustand stores.** Each Tauri window is
  its own JavaScript context, so `useAppStore` instances are *not*
  shared between windows. Any state that must be coherent across
  windows needs to round-trip via a Tauri command.

## Common tasks (with the right entry point)

| Task | Where to start |
|---|---|
| Add a new Tauri command | `src-tauri/src/commands.rs` + register in `lib.rs` + add typed wrapper in `src/lib/tauri.ts` |
| Add a SQLite table | Append to `MIGRATIONS` in `src-tauri/src/db/schema.rs` (never edit a past migration). Update the `fresh_migration_creates_all_tables` test. |
| Add a new legendary | Append to `src-tauri/data/legendary_collections.json`. Verify achievement ids against `https://api.guildwars2.com/v2/achievements?ids=...`. |
| Link an achievement to a boss | Append to `src-tauri/data/achievement_boss_links.json`. The boss id must match `boss_schedule.json`. |
| Add a hotkey | `src/hooks/useHotkeys.ts`. Add the const at the top + the `register` call inside `setup`. |
| Add an appearance setting | Extend `AppearanceSettings` in `commands.rs` + the matching TS type in `src/types/gw2.ts` + a slider in `src/components/SettingsPanel.tsx`. |
| Debug a sync issue | Set `$env:RUST_LOG = "info,gw2_overlay_lib=debug"` before `npm run tauri dev` to see the sync engine's tracing logs. |

## Things to NOT do without a strong reason

- Don't replace `rusqlite` with `sqlx`. The bundled feature gives us a
  zero-DLL Windows build; switching would require a build-time SQL
  schema discovery that doesn't fit a single-user app.
- Don't move from Zustand to Redux/Jotai. Same shape, more boilerplate.
- Don't add `localStorage` or browser-storage features behind React.
- Don't widen the IPC surface unnecessarily. Each new `cmd_*` is a
  permission surface and a TypeScript binding to maintain.
- Don't edit a past migration. Append a new one.
- Don't fetch from the GW2 API outside of `api/client.rs`. The
  rate-limiter only works if everyone goes through the shared
  `ApiClient`.

## Pre-commit ritual

```powershell
# in src-tauri/
cargo test --lib                                        # 55+ unit tests
cargo clippy --all-targets -- -D warnings               # zero warnings

# in repo root
npm run build                                           # tsc strict + Vite
```

If any of those fail, fix before committing. Integration tests
(`cargo test --test api_integration -- --ignored`) require a real
GW2_API_KEY env var and are not part of the pre-commit ritual.
