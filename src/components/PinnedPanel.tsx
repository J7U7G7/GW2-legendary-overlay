import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import type { PinnedItem, UpcomingEvent } from "../types/gw2";

function formatCountdown(targetIso: string, now: number): string {
  const diff = new Date(targetIso).getTime() - now;
  if (diff <= 0) return "now";
  const sec = Math.floor(diff / 1000);
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}`;
  return `${m}m`;
}

function ProgressBar({ ratio }: { ratio: number }) {
  return (
    <div className="h-1 w-full bg-white/10 rounded overflow-hidden">
      <div
        className="h-full bg-[var(--accent-color)]"
        style={{ width: `${Math.round(ratio * 100)}%` }}
      />
    </div>
  );
}

function WaypointButton({ event }: { event: UpcomingEvent }) {
  const [copied, setCopied] = useState(false);
  if (!event.waypoint_code) return null;
  const onClick = async () => {
    try {
      await navigator.clipboard.writeText(event.waypoint_code!);
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
      title="Copy waypoint code to clipboard"
    >
      {copied ? "✓ copied" : "📋 WP"}
    </button>
  );
}

function PinnedItemRow({ item, now }: { item: PinnedItem; now: number }) {
  const unpin = useAppStore((s) => s.unpin);
  const ratio = item.completion_ratio;
  const ratioLabel =
    item.current !== null && item.max !== null && item.max > 0
      ? `${item.current}/${item.max}`
      : item.done
        ? "done"
        : "0%";

  const mins = item.next_event
    ? Math.max(0, Math.floor((new Date(item.next_event.start_at).getTime() - now) / 60000))
    : null;
  const urgentBand =
    !item.done && mins !== null && mins <= 10
      ? "bg-amber-400/20 border-l-2 border-amber-300"
      : !item.done && item.score >= 50
        ? "bg-amber-400/10 border-l-2 border-amber-300/50"
        : "border-l-2 border-transparent";

  const nameClass = item.done ? "opacity-40" : "opacity-95";

  return (
    <li className={`px-3 py-1.5 border-b border-white/5 ${urgentBand}`}>
      <div className="flex items-center justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span
              className={
                item.done
                  ? "text-[var(--accent-color)] opacity-50"
                  : ratio > 0
                    ? "text-amber-300"
                    : "opacity-60"
              }
            >
              {item.done ? "✓" : ratio > 0 ? "◐" : "○"}
            </span>
            <span className={`${nameClass} truncate`} title={item.name}>
              {item.name}
            </span>
          </div>
          {item.next_event && !item.done && (
            <div className="ml-5 mt-0.5 text-[11px] flex items-center gap-1.5 flex-wrap">
              <span
                className={
                  mins !== null && mins <= 10
                    ? "text-amber-300 font-semibold"
                    : "text-amber-200/80"
                }
              >
                ⏰ {item.next_event.name}
              </span>
              <span className="font-mono opacity-80">in {formatCountdown(item.next_event.start_at, now)}</span>
              <span className="opacity-50">·</span>
              <span className="opacity-70 truncate">📍 {item.next_event.map}</span>
              <WaypointButton event={item.next_event} />
            </div>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <span className="font-mono text-[10px] opacity-60">{ratioLabel}</span>
          <button
            type="button"
            onClick={() => void unpin(item.id)}
            className="opacity-40 hover:opacity-100 text-xs"
            title="Unpin"
          >
            ✕
          </button>
        </div>
      </div>
      {!item.done && item.max !== null && item.max > 0 && (
        <div className="ml-5 mt-1">
          <ProgressBar ratio={ratio} />
        </div>
      )}
    </li>
  );
}

export function PinnedPanel() {
  const pinned = useAppStore((s) => s.pinned);
  const setView = useAppStore((s) => s.setView);

  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  // Sort: active (not done) items by score desc first, then done items at the bottom.
  const sorted = useMemo(() => {
    const active = pinned.filter((p) => !p.done);
    const done = pinned.filter((p) => p.done);
    return [...active, ...done];
  }, [pinned]);

  if (pinned.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-xs opacity-60 gap-3 px-4 text-center">
        <p>No pinned achievements yet.</p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setView("catalog")}
            className="px-2 py-1 text-xs bg-white/10 hover:bg-white/20 rounded"
          >
            Browse legendaries
          </button>
          <button
            type="button"
            onClick={() => setView("search")}
            className="px-2 py-1 text-xs bg-white/10 hover:bg-white/20 rounded"
          >
            Search
          </button>
        </div>
      </div>
    );
  }

  return (
    <ul className="flex-1 overflow-y-auto">
      {sorted.map((item) => (
        <PinnedItemRow key={item.id} item={item} now={now} />
      ))}
    </ul>
  );
}
