import { useState } from "react";

import type { WizardsVaultObjective, WizardsVaultPeriod } from "../types/gw2";

type Props = {
  label: string;
  period: WizardsVaultPeriod | null;
};

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

export function WizardsVaultPanel({ label, period }: Props) {
  const [open, setOpen] = useState(true);

  if (!period || period.objectives.length === 0) {
    return (
      <div className="px-3 py-1.5 text-xs opacity-50">
        ▸ {label}: <span className="italic">no data yet</span>
      </div>
    );
  }

  const done = period.objectives.filter(
    (o) => o.progress_current >= o.progress_complete,
  ).length;

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
        <ul className="mt-1 ml-1">
          {period.objectives.map((o) => (
            <ObjectiveRow key={o.id} o={o} />
          ))}
        </ul>
      )}
    </div>
  );
}
