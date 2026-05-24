import { useEffect, useState } from "react";

import { useAppStore } from "../store/app";
import type { PinnedItem } from "../types/gw2";

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

function PinnedItemRow({ item, now }: { item: PinnedItem; now: number }) {
  const unpin = useAppStore((s) => s.unpin);
  const isUrgent = item.score >= 50;
  const ratio = item.completion_ratio;
  const ratioLabel =
    item.current !== null && item.max !== null && item.max > 0
      ? `${item.current}/${item.max}`
      : item.done
        ? "done"
        : "0%";

  return (
    <li
      className={`px-3 py-1.5 border-b border-white/5 ${isUrgent ? "bg-amber-400/5" : ""}`}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span
              className={
                item.done
                  ? "text-[var(--accent-color)]"
                  : ratio > 0
                    ? "text-amber-300"
                    : "opacity-60"
              }
            >
              {item.done ? "✓" : ratio > 0 ? "◐" : "○"}
            </span>
            <span
              className={
                item.done ? "opacity-50 line-through truncate" : "opacity-95 truncate"
              }
              title={item.name}
            >
              {item.name}
            </span>
          </div>
          {item.next_event && (
            <div className="ml-5 text-[10px] opacity-70 flex items-center gap-1">
              <span className={isUrgent ? "text-amber-300 font-semibold" : ""}>
                ⏰ {item.next_event.name}
              </span>
              <span className="font-mono">in {formatCountdown(item.next_event.start_at, now)}</span>
              <span className="opacity-60">· 📍 {item.next_event.map}</span>
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
      {!item.done && item.max && item.max > 0 && (
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
      {pinned.map((item) => (
        <PinnedItemRow key={item.id} item={item} now={now} />
      ))}
    </ul>
  );
}
