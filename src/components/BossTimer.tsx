import { useEffect, useState } from "react";

import type { UpcomingEvent } from "../types/gw2";

function formatCountdown(targetIso: string, now: number): string {
  const target = new Date(targetIso).getTime();
  const diffMs = target - now;
  if (diffMs <= 0) return "now";
  const totalSec = Math.floor(diffMs / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

type Props = {
  next: UpcomingEvent | null;
};

export function BossTimer({ next }: Props) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  if (!next) {
    return (
      <div className="px-3 py-2 text-xs opacity-50 border-b border-white/10">
        No upcoming events in the next 3 hours.
      </div>
    );
  }

  return (
    <div className="px-3 py-2 text-xs border-b border-white/10 flex items-center gap-2">
      <span className="text-[var(--accent-color)] font-semibold">⏰ {next.name}</span>
      <span className="opacity-60">in</span>
      <span className="font-mono">{formatCountdown(next.start_at, now)}</span>
      <span className="opacity-50">·</span>
      <span className="opacity-70 truncate">📍 {next.map}</span>
    </div>
  );
}
