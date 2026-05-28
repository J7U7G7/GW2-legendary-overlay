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
