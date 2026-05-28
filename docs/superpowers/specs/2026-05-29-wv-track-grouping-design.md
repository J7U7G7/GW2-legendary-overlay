# Wizard's Vault — group objectives by track

- **Date:** 2026-05-29
- **Status:** Design approved, pending implementation plan
- **Scope:** The `Wv` tab of the main overlay window. Group each WV
  period's objectives by `track` (game mode) into collapsible
  sections and surface the still-incomplete ("missing") ones.

## Problem

The `Wv` tab renders three panels — Daily / Weekly / Special — each a
single flat list of objectives (`WizardsVaultPanel.tsx`). With ~7-10
daily objectives spanning PvE / PvP / WvW, the user can't quickly see
"what daily PvE work is left." There is no categorisation and no
separation of done vs. remaining beyond per-row styling.

The classic per-expansion daily achievements (`/v2/achievements/daily`)
that some players expect are **not an option**: that endpoint has
returned 503 since the Wizard's Vault rollout (see project memory
`gw2_api_daily_deprecated`). The WV objective payload carries no
expansion field. The only categorisation axis the API provides is
`track` (the game mode), which is already present on every objective.

## Approach (chosen: A — frontend-only grouping by `track`)

`WizardsVaultObjective.track` is already on each objective in the TS
types and is already used as the row tooltip. Group purely in
`WizardsVaultPanel.tsx`. **No Rust / DB / command / type changes** —
the data already contains everything needed.

Rejected: **B (backend-enriched grouping)** — would touch the sync
table, command, and types to return a pre-grouped shape for zero gain,
since `track` is already per-objective on the client.

This mirrors the legendary tier 3 pattern (collapsible sub-sections,
incomplete-first, completed collapsed) for UI consistency.

## Component design (`src/components/WizardsVaultPanel.tsx`)

The component is reused for all three panels (Daily / Weekly /
Special), so grouping applies uniformly to all three.

### `groupByTrack` — pure helper (exported)

```
groupByTrack(objectives: WizardsVaultObjective[]): { track: string; objectives: WizardsVaultObjective[] }[]
```

- Buckets objectives by their `track` string.
- Emits buckets in a fixed order: `PvE`, `PvP`, `WvW`, `Fractals`.
- Any objective whose `track` is empty/unknown (not in that list)
  goes into a trailing `"Other"` bucket.
- Empty buckets are omitted (a period with no PvP objectives shows no
  PvP section).
- Exported as a standalone function so it is unit-testable later if a
  JS test runner is ever added (none today — see Testing).

### `TrackSection` — new sub-component

Renders one track bucket:

- **Header (collapsible):** `track` label + `done/total` count for
  that track (done = `progress_current >= progress_complete`).
- **Default expansion:** a section with any incomplete objective
  starts **expanded** (surfacing the "missing" work); a fully-complete
  section starts **collapsed** with a ✓ and renders no rows.
- **Row ordering inside a section:** incomplete objectives first
  (the "missing" ones), then complete/claimed objectives.
- **Row rendering:** reuse the existing `ObjectiveRow` (its
  claimed/done/in-progress icon + strike-through styling is kept
  as-is).
- Local collapse state via `useState` (no persistence), matching the
  existing panel-level collapse and the tier 3 group sections.

### `WizardsVaultPanel` — wiring

- Keep the existing panel-level header (label + global `done/total`)
  and its top-level collapse (default open).
- The `!period || objectives.length === 0` → "no data yet" branch is
  unchanged.
- Replace the flat `<ul>` of `ObjectiveRow` with a list of
  `TrackSection`, one per non-empty bucket from `groupByTrack`.

This produces a two-level structure: period panel → track sections →
objective rows.

## Edge cases

- **Empty / null period:** existing "no data yet" message, unchanged.
- **Unknown or empty `track`:** falls into the `"Other"` bucket
  (rendered last). No crash, no dropped objective.
- **Fully-complete track section:** collapsed with ✓, no rows.
- **Fully-complete period:** every section collapsed; the panel-level
  `done/total` still shows the totals.

## Testing

The repo has **no JavaScript test runner** (gates are Rust
`cargo test` + `npm run build` tsc-strict; the current
`WizardsVaultPanel` has no tests). Consistent with that:

- `groupByTrack` is written as an exported pure function for clarity
  and future testability, but no JS test harness is added (YAGNI).
- Verification: `npm run build` (tsc strict + Vite) must pass, plus a
  manual smoke test on the `Wv` tab confirming sections appear per
  track, incomplete-first ordering, and completed-section collapse.

## Out of scope

- Any backend / sync / DB / command / type change.
- Title→expansion mapping (the API provides no expansion data).
- Persisting collapse state across sessions.
- Reordering or filtering the period panels themselves.
