# Pivots history

The project shipped three substantial product/architecture pivots during
its first iteration cycle. Documenting them here so future contributors
don't have to re-derive the *why* from the git log.

## Pivot 1: Wizard's Vault first → Legendaries first (commit `6d17c6b`)

**Before:** The original spec (`SPEC-gw2-overlay.md` §5.1, §5.2) described
a daily-achievements-focused overlay: list the day's PvE / PvP / WvW /
Fractals dailies plus the Wizard's Vault objectives, sorted by urgency.
The MVP UI rendered the WV daily/weekly/special objectives prominently.

**Why the pivot:** During the first runtime test, the user gave very
direct feedback: *"je m'en fou un peu des succès de la journée/semaine/
mois c'est pour des collections paramétrables que je souhaite avoir ça
(pour que ça m'aide à faire mes legendaires grosso modo)"*. The real use
case wasn't daily progression — it was *legendary collection tracking*,
which the spec had filed under Phase 2.

**After:** Schema v2 added `pinned_achievements`, `legendary_collections`,
`legendary_collection_members`. A new `Pinned` tab became the primary
view; `WV` was demoted to a secondary tab. A curated catalog of 32
legendaries (`legendary_collections.json`) loaded from disk on boot.

**Bonus discovery during this pivot:** `/v2/achievements/daily` has been
silently broken since the Wizard's Vault rollout (March 2024). ArenaNet
never restored it; queries return 503 permanently. This is documented in
the integration test `legacy_daily_endpoint_is_unavailable` so we'll
notice if they ever fix it. Saved as a project-memory note.

## Pivot 2: Achievement-first pinning → Boss-first pinning (commit `1e1ef87`)

**Before:** The Phase-1 pin model was "pin individual achievements". A
world boss's urgency only mattered as a *signal* attached to an
achievement that was already pinned (Tequatl Slayer achievements got
the green/amber treatment when Tequatl was imminent).

**Why the pivot:** User feedback after testing the legendary flow: *"ça
m'affiche le boss en grisé vu que j'ai tout les succès alors que les
succès pour les worldboss sont secondaires à mes yeux = je veux timer
et waypoints en prio (succès c'est bien mais ça serait mieux en
dépliable avec la liste des succès à faire)"*. The mental model the user
actually had was "I want Tequatl's timer + waypoint as the primary unit;
the achievements are notes attached to it".

**After:** Schema v3 added `pinned_bosses` (boss_id PK). World bosses
became first-class pinnable entities, independent from achievements. The
`PinnedView` IPC payload turned into
`{ boss_groups: [...], standalone: [...] }`. Each boss group renders as
a card with name + countdown + waypoint button as the headline, and the
linked achievements (pulled by `associated_boss` from
`achievement_metadata`) live in a collapsible body. The Events tab was
added so users can pin a boss directly from the schedule.

## Pivot 3: One window → Two windows (commit `a612816`)

**Before:** All five tabs (Pinned / Events / Catalog / Search / WV)
lived in a single 380 × 600 window.

**Why the pivot:** *"ça serait bien d'avoir 2 fenetres une pour les
worldboss et event et une pour les succès pinned = on y verra plus
clair -> toujours avec des fenetres parametrable et persistant"*. With
many pinned items it became cramped, and the user wanted to lay them
out side by side (e.g. events strip on the side, pinned column in the
middle).

**After:** Two Tauri windows declared in `tauri.conf.json`. Each loads
`index.html` with a different URL fragment; `App.tsx` reads
`getCurrentWindow().label` and routes to either `<Overlay/>` or
`<EventsWindow/>`. The window-state plugin restores position and size
independently for each label.

**Likely upcoming Pivot 4 (open):** *"3 fenetres au total"* — splitting
the main Pinned panel into two more windows (one for pinned bosses,
one for pinned collections/achievements). Discussed but not yet
implemented. See [ROADMAP.md](ROADMAP.md).

---

## Bugs/findings that shaped the project

Smaller course-corrections that aren't full pivots but worth knowing
about so you don't relearn them the hard way:

- **`tokio::spawn` from Tauri setup hook panics** (`fix(sync)
  f99a688`). Tauri 2's `setup` callback isn't inside the tokio runtime
  context. Use `tauri::async_runtime::spawn` instead. This bit us once
  and will bite anyone who adds a new background task.

- **`tauri-plugin-window-state` 2.4 doesn't auto-restore**
  (`fix(window) be4ad62`). You must call `WindowExt::restore_state` in
  `setup` for every window, and you must NOT declare `x`/`y` in
  `tauri.conf.json` — those win over the restore.

- **React's `data-tauri-drag-region={false}` still renders the
  attribute** (`fix(ui) 36c439e`). Tauri's drag detection considers the
  attribute *present* regardless of value. To exclude an element from
  drag, omit the attribute entirely.

- **`bit.text ?? "fallback"` doesn't catch empty strings**
  (`fix(items) e7e56a4`). The GW2 API returns `text: ""` for many bits;
  `??` only short-circuits on null/undefined. Use a
  `text.trim().length > 0` check.

- **GW2 API's `requirement` field has missing tier-count
  substitution** (`feat(pinned) 1dfd584`). The API returns strings like
  `"Win  costume brawls"` with a literal double space where the tier
  count should appear in-game. Frontend `fillRequirement(req, max)` does
  the substitution.

- **GW2 API descriptions contain in-game markup tags**
  (`feat(ui) e7e56a4`). `<c=@flavor>...</c>`, `<br>` and friends leak
  through. `stripGw2Markup` cleans them at display time.

- **Item descriptions use `requirement` with `  ` ditto + bits with
  `text: ""`** (`feat(items) a33ec8d`). To show "Inquest Dragon Energy
  Research Cube" instead of "Item #74878" we fetch `/v2/items?ids=...`
  into the `items_cache` table after every pin. The frontend's `pin()`
  store action calls `warmItemCache()` after each pin so the names
  resolve within one round trip.

- **Window-state plugin saves on close, not on SIGINT
  (Ctrl+C in dev terminal).** Pressing Ctrl+C in the `tauri dev`
  terminal kills the process without window close events firing, so
  position changes get lost. Use a Quit button in the UI (added in a
  follow-up commit — see commit log) or close via the OS title-bar
  controls if you re-enable decorations.
