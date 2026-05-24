import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import type { EventView } from "../types/gw2";

function formatCountdown(targetIso: string, now: number): string {
  const diff = new Date(targetIso).getTime() - now;
  if (diff <= 0) return "now";
  const sec = Math.floor(diff / 1000);
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}`;
  return `${m}m`;
}

function WaypointButton({ code }: { code: string | null }) {
  const [copied, setCopied] = useState(false);
  if (!code) return null;
  const onClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };
  return (
    <button
      type="button"
      onClick={onClick}
      className="px-1 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20 font-mono"
      title="Copy waypoint code"
    >
      {copied ? "✓" : "📋"}
    </button>
  );
}

function EventRow({ event, now }: { event: EventView; now: number }) {
  const pinBoss = useAppStore((s) => s.pinBoss);
  const unpinBoss = useAppStore((s) => s.unpinBoss);
  const mins = Math.max(0, Math.floor((new Date(event.next_spawn).getTime() - now) / 60000));
  const isImminent = mins <= 10;

  return (
    <li
      className={`px-3 py-1 flex items-center justify-between gap-2 border-b border-white/5 ${isImminent ? "bg-amber-400/10" : ""}`}
    >
      <div className="flex-1 min-w-0">
        <div className={`truncate text-xs ${isImminent ? "text-amber-300 font-semibold" : ""}`}>
          {event.name}
        </div>
        <div className="text-[10px] opacity-60 truncate">📍 {event.map}</div>
      </div>
      <div className="flex items-center gap-1.5 shrink-0">
        <span className="font-mono text-[10px] opacity-80">{formatCountdown(event.next_spawn, now)}</span>
        <WaypointButton code={event.waypoint_code} />
        {event.pinned ? (
          <button
            type="button"
            onClick={() => void unpinBoss(event.id)}
            className="px-1.5 py-0.5 text-[10px] text-[var(--accent-color)]"
            title="Unpin boss"
          >
            ✓
          </button>
        ) : (
          <button
            type="button"
            onClick={() => void pinBoss(event.id)}
            className="px-1.5 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
          >
            +
          </button>
        )}
      </div>
    </li>
  );
}

export function EventsTab() {
  const events = useAppStore((s) => s.events);

  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  // Group by expansion
  const groups = useMemo(() => {
    const map = new Map<string, EventView[]>();
    for (const ev of events) {
      const arr = map.get(ev.expansion) ?? [];
      arr.push(ev);
      map.set(ev.expansion, arr);
    }
    return Array.from(map.entries());
  }, [events]);

  if (events.length === 0) {
    return <div className="flex-1 px-3 py-2 text-xs opacity-50 italic">Loading events…</div>;
  }

  return (
    <div className="flex-1 overflow-y-auto text-xs">
      {groups.map(([expansion, evs]) => (
        <section key={expansion}>
          <h3 className="sticky top-0 px-3 py-1 text-[10px] font-semibold uppercase tracking-wider bg-black/60 backdrop-blur opacity-80 border-b border-white/10">
            {expansion}
          </h3>
          <ul>
            {evs.map((ev) => (
              <EventRow key={ev.id} event={ev} now={now} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
