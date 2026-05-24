# Pivots history

The project shipped four substantial product/architecture pivots
during its first iteration cycle. Documenting them here so future
contributors don't have to re-derive the *why* from the git log.

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
