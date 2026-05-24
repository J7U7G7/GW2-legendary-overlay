# Roadmap

What's planned but not yet built. Items are grouped by scope. The order
inside a section is rough priority but not a strict commitment — pick
based on user feedback.

## Active backlog (user-requested, scoped)

### 3-window split

Currently the overlay is two windows: **main** (tabs Pinned / Catalog
/ Search / WV) and **events** (the boss + meta feed in its own
movable window).

The next iteration splits the Pinned tab into two more windows so the
user can lay them out independently:

- **Main / Configure** — Events tab + Catalog + Search + WV + Settings.
  This is the "set up what I want to track" window; not consulted
  during active play.
- **Pinned Bosses** — only the boss groups from `PinnedView`. Tight
  vertical strip with countdown + waypoint + collapsible achievement
  list per boss.
- **Pinned Achievements** — only the standalone (boss-less) pins:
  legendary collection steps, raid achievements, etc.

Estimated work: a few hundred lines of frontend + a third entry in
`tauri.conf.json` + `restore_state` loop. The IPC payloads already
separate `boss_groups` from `standalone`, so no backend change is
needed beyond the new window declaration.

### Window collapse to thin bar

Each window gets a `_` (or `▾`) button that hides everything but the
header. Useful during active play — keep the overlay visible (so the
hotkeys work and the user can see the boss header line) without it
eating screen real estate.

Implementation: pure React state in each window component, plus a
shorter window height when collapsed (use `getCurrentWindow().setSize`
or just rely on the body content collapsing). Persist the collapsed
state per window in the existing `settings` table.

### Configurable hotkeys from the Settings panel

`useHotkeys.ts` currently hard-codes the three shortcuts. Add UI to
let the user pick custom key combos (validate against
`isRegistered` + `unregister` + `register` to swap them at runtime).
Persist via `appearance`-style JSON in `settings`.

### Configurable notification lead time + test button

Currently `BOSS_NOTIFY_LEAD_MINUTES = 2` is hardcoded in
`sync/engine.rs`. Make it a setting (1–15 min slider). Add a "Test
notification" button in `SettingsPanel` so the user can verify the
toast pipeline works without waiting for a real spawn.

### Meta event notifications

The boss-watcher loop only walks `pinned_bosses` (world bosses). The
same logic should apply to pinned meta events: notify when a phase
they care about (e.g. Soo-Won at the end of Dragon's End) is about to
start. The data is already in `schedule.meta_events`.

### More boss-link mappings

Today's `achievement_boss_links.json` covers Tequatl, Triple Trouble,
Shatterer — every world boss that has a dedicated achievement
category in the GW2 API. The other 10 bosses (Karka Queen, Claw of
Jormag, …) don't have categories; they only show up in retired daily
PvE achievements that the GW2 API doesn't expose.

If we want them, the path is manual research from the wiki (e.g.
"Karka Queen Killer" sits under some other category) + augmenting the
mapping by hand.

## Aspirational backlog (large features, no commitment)

Big features that have been discussed but not designed in detail. Each
is its own project-scale effort and should get its own design pass
before any code starts.

### Smart legendary selection

> *"Selection de légendaires à faire et optimisation de ce qu'il y'a à
> faire à partir de mon compte, mes succès, mes items"*

Given the user's current `account_progress` + `bank` + `inventory` +
`wallet`, recommend the legendary that's closest to completion. Cross-
reference what they already own (precursor in bank, partial gift
collections, currencies) with the requirements of each legendary
collection, then rank by remaining-effort.

Data sources we'd need beyond what we cache today:
- `/v2/account/bank`
- `/v2/account/inventory`
- `/v2/account/materials`
- `/v2/account/wallet`
- Recipe data: `/v2/recipes` + wiki recipes for the "Gift of …" components
- Mystic Forge inputs

Complex enough to be a project on its own.

### Pathing (TaCo-style markers)

> *"TaCo et ses marqueurs mais en mieux, tous les pack de marqueurs sont
> disponible simplement et peuvent être mis à jour automatiquement."*

In-world POI markers rendered by reading the GW2 Mumble API position
stream and drawing on the overlay. Requires:

- Reading Mumble shared memory (`MumbleLink` block) for player coords.
- Loading `.taco` marker packs (XML format).
- Rendering 3D-projected markers — this means the overlay has to
  render *behind* the cursor but *over* the game, with per-marker
  occlusion. Likely needs a transparent click-through layer separate
  from the current React UI, perhaps a WGPU canvas.

This is a substantial scope addition. Consider whether linking the
existing BlishHUD/ArcDPS markers via inter-process plugin is a saner
path than re-implementing.

### Builds Manager

> *"Un manager de build GW2 avec les builds de snowcrows inclus."*

Catalog of meta builds (Snowcrows data dump), pin a build per
character, copy-paste chat code to the in-game build storage. Snowcrows
publishes JSON-like dumps that can be scraped or fetched if they
expose an API; otherwise it's a manual data effort.

### Mounts radial menu

> *"Un menu radial de montures très pratique pour passer très vite de
> l'une à l'autre"*

A circular hotkey palette that, when triggered (e.g. mouse-button
side-button), pops up under the cursor and lets the user click a
mount icon to invoke its keybind. GW2 Radial does this; doing it
"better" means tighter latency, configurable icons, and remembering
the last-used mount per zone.

### Item Search

> *"Permet de chercher un item sur tout votre compte. (GW2 Efficiency
> mais en mieux)"*

Search-by-name across `/v2/account/bank`, `/v2/account/inventory`,
`/v2/account/materials`, all characters' bags, and the wallet. Result
list shows "where it is + how many". The `items_cache` we already
maintain partially covers this — would need to extend the cache to
all items the account has touched.

### Daily/Weekly todos

> *"Permet de créer une liste de chose à faire qui se réset soit tous
> les jours soit toutes les semaines"*

Free-form text todos with a reset schedule (00:00 UTC daily, Monday
07:30 UTC weekly). Simple table; just need a `todos` table + UI.

### Meta + boss notification observer

> *"Vous permet de recevoir des notification pour les Métas et world
> boss."*

Partly built (the `sync::engine::spawn_boss_watcher_loop`). Extending
it to metas + adding the lead-time setting + meta selection UI gets
us to feature parity with the linked feature request.

## Tooling / housekeeping

- `tauri build` MSI installer flow + CI.
- A `--reset` flag or a settings-panel button to wipe the SQLite file
  (useful for re-running the bulk sync after spec changes).
- A `feature_flag` table for staged rollouts of new behaviors.
