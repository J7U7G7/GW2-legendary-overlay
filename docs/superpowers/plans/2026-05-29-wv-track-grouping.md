# Wizard's Vault Track Grouping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group each Wizard's Vault period's objectives by `track` (game mode) into collapsible sections in the `Wv` tab, surfacing the still-incomplete ("missing") ones.

**Architecture:** Frontend-only change to `src/components/WizardsVaultPanel.tsx`. The objective `track` is already on each `WizardsVaultObjective` client-side, so a pure `groupByTrack` helper plus a new `TrackSection` sub-component is all that's needed. The component is reused for Daily/Weekly/Special, so all three panels get grouping. No Rust/DB/command/type changes.

**Tech Stack:** React + TypeScript (strict), Tailwind, Vite.

**Spec:** `docs/superpowers/specs/2026-05-29-wv-track-grouping-design.md`

---

## File Structure

- `src/components/WizardsVaultPanel.tsx` — the only file. Gains: a `TRACK_ORDER`/`OTHER_TRACK` constant, an `isDone` helper, an exported pure `groupByTrack` function, and a `TrackSection` component. `WizardsVaultPanel` is rewired to render track sections instead of a flat list. `ObjectiveRow` is unchanged.

**Testing note:** the repo has **no JavaScript test runner** (gates are Rust `cargo test` + `npm run build` tsc-strict; this component has no existing tests). Verification for this plan is `npm run build` (tsc strict + Vite) plus a manual smoke test. `groupByTrack` is exported as a pure function for clarity and future testability, but no JS test harness is added (YAGNI, per spec).

---

## Task 1: Group Wizard's Vault objectives by track

**Files:**
- Modify: `src/components/WizardsVaultPanel.tsx` (full rewrite of the file; `ObjectiveRow` body preserved verbatim)

- [ ] **Step 1: Replace the file contents**

Overwrite `src/components/WizardsVaultPanel.tsx` with exactly:

```tsx
import { useState } from "react";

import type { WizardsVaultObjective, WizardsVaultPeriod } from "../types/gw2";

type Props = {
  label: string;
  period: WizardsVaultPeriod | null;
};

/** Fixed display order for objective tracks; anything else buckets into "Other". */
const TRACK_ORDER = ["PvE", "PvP", "WvW", "Fractals"];
const OTHER_TRACK = "Other";

function isDone(o: WizardsVaultObjective): boolean {
  return o.progress_current >= o.progress_complete;
}

/**
 * Bucket objectives by `track` in TRACK_ORDER, with unknown/empty tracks last
 * under "Other". Empty buckets are omitted.
 */
export function groupByTrack(
  objectives: WizardsVaultObjective[],
): { track: string; objectives: WizardsVaultObjective[] }[] {
  const buckets = new Map<string, WizardsVaultObjective[]>();
  for (const o of objectives) {
    const key = TRACK_ORDER.includes(o.track) ? o.track : OTHER_TRACK;
    const arr = buckets.get(key) ?? [];
    arr.push(o);
    buckets.set(key, arr);
  }
  return [...TRACK_ORDER, OTHER_TRACK]
    .filter((t) => buckets.has(t))
    .map((t) => ({ track: t, objectives: buckets.get(t)! }));
}

function ObjectiveRow({ o }: { o: WizardsVaultObjective }) {
  const ratio =
    o.progress_complete === 0 ? 0 : Math.min(1, o.progress_current / o.progress_complete);
  const icon = o.claimed ? "✓" : o.progress_current >= o.progress_complete ? "◉" : ratio > 0 ? "◐" : "○";
  return (
    <li className="grid grid-cols-[1rem_1fr_auto] gap-2 items-center py-0.5">
      <span
        className={
          o.claimed
            ? "text-[var(--accent-color)]"
            : o.progress_current >= o.progress_complete
              ? "text-amber-300"
              : "opacity-60"
        }
      >
        {icon}
      </span>
      <span
        className={
          o.claimed ? "opacity-50 line-through" : "opacity-90"
        }
        title={o.track}
      >
        {o.title}
      </span>
      <span className="opacity-50 font-mono text-[10px]">
        {o.progress_current}/{o.progress_complete}
      </span>
    </li>
  );
}

function TrackSection({
  track,
  objectives,
}: {
  track: string;
  objectives: WizardsVaultObjective[];
}) {
  const done = objectives.filter(isDone).length;
  const complete = done === objectives.length;
  const [open, setOpen] = useState(true);
  // Incomplete ("missing") objectives first, completed ones after.
  const ordered = [...objectives].sort((a, b) => Number(isDone(a)) - Number(isDone(b)));

  // A fully-complete track collapses to a static ✓ header with no rows
  // (no dead toggle button).
  if (complete) {
    return (
      <div className="mt-1 flex items-center justify-between text-[11px] opacity-60 px-1">
        <span className="font-semibold">✓ {track}</span>
        <span className="font-mono">
          {done}/{objectives.length}
        </span>
      </div>
    );
  }

  return (
    <div className="mt-1">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center justify-between w-full text-left text-[11px] opacity-80 hover:opacity-100 px-1"
      >
        <span className="font-semibold">
          {open ? "▾ " : "▸ "}
          {track}
        </span>
        <span className="font-mono opacity-60">
          {done}/{objectives.length}
        </span>
      </button>
      {open && (
        <ul className="ml-1">
          {ordered.map((o) => (
            <ObjectiveRow key={o.id} o={o} />
          ))}
        </ul>
      )}
    </div>
  );
}

export function WizardsVaultPanel({ label, period }: Props) {
  const [open, setOpen] = useState(true);

  if (!period || period.objectives.length === 0) {
    return (
      <div className="px-3 py-1.5 text-xs opacity-50">
        ▸ {label}: <span className="italic">no data yet</span>
      </div>
    );
  }

  const done = period.objectives.filter(isDone).length;
  const groups = groupByTrack(period.objectives);

  return (
    <div className="px-3 py-1.5 text-xs">
      <button
        type="button"
        className="flex items-center justify-between w-full text-left font-semibold opacity-90 hover:opacity-100"
        onClick={() => setOpen(!open)}
      >
        <span>
          {open ? "▾" : "▸"} {label}
        </span>
        <span className="font-mono opacity-60">
          {done}/{period.objectives.length}
        </span>
      </button>
      {open && (
        <div className="mt-1 ml-1">
          {groups.map((g) => (
            <TrackSection key={g.track} track={g.track} objectives={g.objectives} />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Build (tsc strict + Vite)**

Run: `npm run build`
Expected: PASS — no type errors. (`groupByTrack` is exported and `TrackSection` consumes it; `ObjectiveRow` unchanged.)

- [ ] **Step 3: Commit**

```bash
git add src/components/WizardsVaultPanel.tsx
git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit -m "feat(wv): group objectives by track in the Wv tab"
```

---

## Task 2: Manual smoke verification (gate only)

**Files:** none

- [ ] **Step 1: Run the app and inspect the Wv tab**

Run: `npm run tauri dev` (needs a configured GW2 API key for live WV data).
Open the **Wv** tab and confirm:
- Each panel (Daily / Weekly / Special) shows sections per track in the order PvE, PvP, WvW, Fractals, with any unknown track under "Other".
- Within a section, incomplete objectives appear before completed/claimed ones.
- A section where everything is done renders as a static `✓ <track>` header with no rows; incomplete sections are expandable (default open).
- An empty/no-key period still shows the "no data yet" message.

- [ ] **Step 2: No commit** (verification only).

---

## Notes for the implementer

- Do not touch any Rust, the sync engine, the DB, or `src/types/gw2.ts` — `track` is already present on `WizardsVaultObjective` (no contract change).
- Keep `ObjectiveRow` exactly as shown; only the surrounding structure changes.
- The `done` count in both the panel header and each `TrackSection` uses the same `isDone` predicate (`progress_current >= progress_complete`), matching the pre-existing panel-header semantics.
- Collapse state is local `useState`, not persisted — matches the existing panel and the legendary tier 3 group sections.
