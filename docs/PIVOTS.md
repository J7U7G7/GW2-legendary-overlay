# Pivots history

The project shipped five substantial product/architecture pivots.
Documenting them here so future contributors don't have to re-derive
the *why* from the git log.

## Pivot 1: Wizard's Vault first → Legendaries first (commit `6d17c6b`)

**Before:** The original spec described a daily-achievements-focused
overlay: list the day's PvE / PvP / WvW / Fractals dailies plus the
Wizard's Vault objectives, sorted by urgency. The MVP UI rendered the
WV daily/weekly/special objectives prominently.

**Why the pivot:** During the first runtime test, the user gave very
direct feedback: *"je m'en fou un peu des succès de la journée/semaine/
mois c'est pour des collections paramétrables que je souhaite avoir ça
(pour que ça m'aide à faire mes legendaires grosso modo)"*. The real
use case wasn't daily progression — it was *legendary collection
tracking*, which the spec had filed under Phase 2.

**After:** Schema v2 added `pinned_achievements`,
`legendary_collections`, `legendary_collection_members`. A new
`Pinned` tab became the primary view; `WV` was demoted to a secondary
tab. A curated catalog of legendaries (`legendary_collections.json`)
loaded from disk on boot.

**Bonus discovery:** `/v2/achievements/daily` has been silently broken
since the Wizard's Vault rollout (March 2024). ArenaNet never restored
it. Documented in the integration test
`legacy_daily_endpoint_is_unavailable`.

## Pivot 2: Achievement-first pinning → Boss-first pinning (commit `1e1ef87`)

**Before:** The Phase-1 pin model was "pin individual achievements". A
world boss's urgency only mattered as a *signal* attached to an
achievement that was already pinned.

**Why the pivot:** User feedback after testing: *"ça m'affiche le boss
en grisé vu que j'ai tout les succès alors que les succès pour les
worldboss sont secondaires à mes yeux = je veux timer et waypoints en
prio"*. The mental model was "I want Tequatl's timer + waypoint as the
primary unit; the achievements are notes attached to it".

**After:** Schema v3 added `pinned_bosses` (boss_id PK). World bosses
became first-class pinnable entities. The `PinnedView` IPC payload
turned into `{ boss_groups: [...], standalone: [...] }`. Each boss
group renders as a card with name + countdown + waypoint button as the
headline, and the linked achievements (pulled by `associated_boss`
from `achievement_metadata`) live in a collapsible body.

## Pivot 3: One window → Two windows (commit `a612816`, superseded)

Briefly tried a two-window split (main + events). Superseded by
**Pivot 4** before stabilising.

## Pivot 4: Three independent windows (commit `0fdd5cd`)

**Why:** *"je veux une fenetre me permettant de set en pinned ce que
je veux (avec tjr GW2events, catalog, search, Wv) + 2 fenetres pinned
(1 fenetre = Worldboss + 1 fenetre pour ce que j'ai pinned en
collection et succès) = 3 fenetres au total"*. The user wanted to lay
their pinned bosses and pinned achievements out side-by-side, each in
their own movable / resizable strip.

**After:**
- `tauri.conf.json` declares three windows: `main`, `bosses`,
  `achievements`. Each loads `index.html` with a different URL hash.
- `App.tsx` routes by `getCurrentWindow().label` to `<Overlay/>`,
  `<BossesWindow/>`, or `<AchievementsWindow/>`.
- `PinnedPanel.tsx` was refactored to export two named views —
  `BossesView` (renders `pinned.boss_groups`) and `AchievementsView`
  (renders `pinned.standalone`). The bosses and achievements windows
  consume those views directly.
- Each window has its own Zustand store + DOM. Cross-window updates
  flow through Tauri events: `pinned_changed` after any pin/unpin/
  boss-remove, `appearance_changed` after settings panel writes.
  Subscribed in every window via the `useCrossWindowSync` hook.
- Secondary windows hide-on-close (intercepted in `lib.rs::
  on_window_event`), reopenable via the 🐉 / 📌 buttons in the main
  header or the Ctrl+Shift+B / Ctrl+Shift+P hotkeys.

**Side fixes that came with this pivot:**
- API key 'disappeared' on launch — root cause was a race where the
  secondary windows' JS fired commands before `app.manage(AppState)`
  had run. Setup now manages an empty state first, then builds the
  engine and slots it in via interior mutability.
- `state.engine` switched from `tokio::sync::Mutex` to `std::sync::
  Mutex` so the setup hook can write to it synchronously.
- `cmd_check_api_key` made tolerant of transient network / 401
  responses; only the periodic sync engine treats `Unauthorized` as
  terminal.

## Pivot 5: FR display → full English (commit `2b004f5`, v0.1.11)

**Before:** Items / skins / achievement bits / WV objectives were
fetched + cached + displayed in French (`?lang=fr`), since the user
plays the FR client. v0.1.10 introduced a parallel `name_en` column +
dual-fetch pipeline to keep the FR display while making wiki links
target the canonical English pages.

**Why the pivot:** The bilingual machinery was working but the
mental-translation tax was real — read "Lingot de mithril" in the
UI, click → land on `wiki/Mithril_Ingot`. Plus the achievement-level
"Open on wiki ↗" link couldn't deep-link (still searched by FR
name) and that gap accumulated frustration. The user explicitly
chose **all-EN** during a brainstorming round, accepting the trade-
off that searching items by FR names (`"bouclier élevé"`) would
no longer work.

**After:** Schema v10 wiped the four FR-cached tables
(items_cache, skins_cache, achievements, wizardsvault) and dropped
the `name_en` columns. Endpoints now fetch `lang=en` directly. The
dual-fetch `tokio::join!` + `name_en` machinery is fully removed.
Bulk re-sync runs once (~50s) on first launch of v0.1.11. Wiki links
now deep-link via `wiki/<EN_name>` directly. Side benefit: the
achievement-level link is no longer broken since `item.name` is now
EN and the existing search URL produces useful results.

**Process note:** This was the first feature shipped via the
superpowers brainstorming → spec → plan → subagent-driven-execution
workflow. The spec + plan documents live at
`docs/superpowers/specs/2026-05-26-full-english-display-design.md`
and `docs/superpowers/plans/2026-05-26-full-english-display.md`.
Worth re-using for future ≥ medium features.

---

## Smaller bugs / findings worth knowing

Course-corrections that aren't full pivots but will bite anyone who
re-derives them:

- **`tokio::spawn` from Tauri setup panics** (`fix(sync) f99a688`).
  Tauri 2's `setup` is sync, not inside the runtime context. Use
  `tauri::async_runtime::spawn`.

- **`tauri-plugin-window-state` 2.4 doesn't auto-restore** (`fix(window)
  be4ad62`). Call `WindowExt::restore_state` for every window in
  `setup`. Don't declare `x`/`y` in `tauri.conf.json` — those win.

- **Ctrl+C in `tauri dev` SIGINTs without firing close events** —
  saved window positions get lost. Added a `⏻` button that calls
  `cmd_save_state_and_quit` (explicit `save_window_state` then
  `app.exit(0)`).

- **React's `data-tauri-drag-region={false}` still renders the
  attribute** (`fix(ui) 36c439e`). Tauri considers the attribute
  present regardless of value. Omit entirely on non-drag children.

- **`bit.text ?? "fallback"` doesn't catch empty strings** — the GW2
  API returns `text: ""` for many bits. Use a `text.trim().length >
  0` check.

- **`requirement` field has missing tier-count substitution** — API
  returns `"Win  costume brawls"` (double space) where the in-game UI
  inserts the count. `fillRequirement(req, max)` does the substitution.

- **API descriptions contain in-game markup tags** —
  `<c=@flavor>...</c>`, `<br>` etc. `stripGw2Markup` cleans them at
  display time.

- **Item names are English by default** — fetched with `?lang=fr` so
  the user can search "bouclier élevé" instead of "Ascended Shield".

- **Character equipment ≠ character bags** — equipped items live in
  `character.equipment[]`, not in `character.bags[]`. Item Search
  missed every equipped piece until that was fixed.

- **The font-size slider was a visual no-op** — every text element
  uses an explicit Tailwind size class, so `body.style.fontSize` does
  nothing. Switched to `zoom` on a `.ui-zoom` div that wraps the
  content area (not the header — buttons must stay visible).

- **CSS variables are per-document** — Settings panel writes
  `--accent-color` to the originating window's root only. The
  `appearance_changed` event triggers `loadSettings()` in every other
  window to apply the same values to their own root.

- **GW2's `/v2/achievements/daily` returns 503 permanently** since
  the Wizard's Vault rollout. Tracked by the integration test
  `legacy_daily_endpoint_is_unavailable`.

- **No generic "world boss daily" achievements exist in the API.**
  ArenaNet retired the daily PvE achievements with WV. Per-boss
  achievement categories exist for Tequatl / Triple Trouble /
  Shatterer only; the other 10 world bosses don't have categories.

- **Snowcrows has no API + their robots.txt blocks AI bots.** The
  Builds Manager ships a curated static JSON; the curator pastes
  chat codes from in-game (Hero panel → Build template → right-click
  → Copy Chat Code).

- **Default-open boss groups** — closing the body of a Tequatl card
  whose achievements were all done made it look "deleted". Cards now
  default-open whenever they have any linked achievements.

- **Zombie table `daily_assignments`** (schema v1, never dropped) —
  was used by the original spec to mirror `/v2/achievements/daily`.
  That endpoint has returned 503 since the Wizard's Vault rollout
  (Pivot 1), so the table has zero readers and zero writers in the
  current code. Can't drop without editing past migration v1, so it
  stays as harmless empty schema. Audit grep target: `daily_assignments`
  appears only inside its own `CREATE TABLE` string + the
  `fresh_migration_creates_all_tables` test list.

- **Tauri 2 multi-window boot race** (`fix(api) 8d7af81`, v0.1.9) —
  with multiple windows declared in `tauri.conf.json`, the WebView2
  webviews boot in parallel with the setup hook. The main window's
  React `useEffect` can fire `cmd_check_api_key` BEFORE
  `app.manage(AppState)` has run, getting `"state not managed for
  field state on command cmd_check_api_key"`. The error was caught
  by the FE store and persisted as "no API key" → user saw the
  setup screen repeatedly despite the key being in DB. Fix: every
  Tauri invoke goes through a `lib/tauri.ts::invoke` wrapper that
  retries up to 6 times with exponential backoff (80 ms × attempt)
  on the specific `"state not managed"` error string. Other errors
  propagate immediately.

- **Tauri 2 auto-updater needs `bundle.createUpdaterArtifacts: true`**
  — configuring `plugins.updater.pubkey` is NOT sufficient to make
  `tauri build` emit `.sig` files alongside the installers.
  Documented in passing under "Migrating from v1" but missing from
  the updater plugin page. Without this, the .exe + .msi land in
  the bundle dir but no signatures, and the manifest-gen step in
  the release workflow fails with "NSIS installer or signature not
  found".

- **GitHub Release asset URLs replace spaces with dots**
  (`fix(release) 1e85091`) — `tauri build` produces
  `"GW2 Legendary Overlay_0.1.X_x64-setup.exe"` but the GitHub
  download URL uses `"GW2.Legendary.Overlay_0.1.X_x64-setup.exe"`.
  The `latest.json` manifest URL must mirror that transformation or
  the in-app updater hits a 404 on install.

- **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` cannot be empty on GitHub
  Secrets** — the UI rejects an empty value. Workaround: hardcode
  the env var to `""` in `release.yml` and ignore whatever the
  secret contains. If the signing key was generated with
  `--password ""` (unencrypted), minisign IS strict about the
  password match → passing any non-empty value would fail signing
  silently.

- **NSIS `installMode: "passive"`** — required for the in-app
  auto-updater to work without UAC re-prompts per update. The
  default `wizard` mode interrupts the user mid-flow.

- **`load_api_key` is the load-bearing function** — every diagnostic
  we added for the "API key not persisting" bug (which turned out to
  be Pivot 5's boot race, not DPAPI) flows through this function.
  Per-step logging lives there: "no row in settings" / "base64
  decode failed" / "DPAPI unprotect failed" / "loaded successfully".
  Keep those logs intact even if you refactor — they're how the next
  bug gets diagnosed without an in-person debugging session.

- **FE → tracing log bridge** (`cmd_log_event`) — React can write to
  the same rolling file log via this command. Used in `useHotkeys`,
  `store.checkApiKey`, `Overlay.render`. Critical for diagnosing
  production-only bugs that `console.log` can't surface (devtools
  are awkward in a WebView2 release build).

- **`is_stale` must check `count(*) = 0`** (commit `35267227`,
  Pivot 5) — the `last_full_sync` timestamp lives in the `settings`
  table which survives a data-table wipe. Without the count check,
  a Schema v10–style wipe would leave the bulk re-fetch dormant
  forever. Apply the same pattern to any future similar staleness
  check.

- **Tauri-bundler bundle dir layout** — installers land at
  `src-tauri/target/release/bundle/{msi,nsis}/`. The `latest.json`
  manifest expects to be uploaded alongside, not in those subdirs.
  Workflow generates it in the repo root.
