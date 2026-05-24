import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import type { PinnedBossGroup, PinnedItem } from "../types/gw2";

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
      className="px-1.5 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20 font-mono"
      title="Copy waypoint chat code"
    >
      {copied ? "✓ copied" : "📋 WP"}
    </button>
  );
}

function AchievementRow({ item }: { item: PinnedItem }) {
  const unpin = useAppStore((s) => s.unpin);
  const ratio = item.completion_ratio;
  const ratioLabel =
    item.current !== null && item.max !== null && item.max > 0
      ? `${item.current}/${item.max}`
      : item.done
        ? "done"
        : "0%";

  return (
    <li className="pl-6 pr-3 py-1 flex items-center justify-between gap-2 text-[11px] border-t border-white/5">
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
          <span className={item.done ? "opacity-40 truncate" : "opacity-95 truncate"} title={item.name}>
            {item.name}
          </span>
        </div>
        {!item.done && item.max !== null && item.max > 0 && (
          <div className="ml-5 mt-0.5">
            <ProgressBar ratio={ratio} />
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
    </li>
  );
}

function BossGroupCard({ group, now }: { group: PinnedBossGroup; now: number }) {
  // Default open when there are remaining achievements; closed otherwise.
  const [open, setOpen] = useState(group.has_remaining);
  const unpinBoss = useAppStore((s) => s.unpinBoss);

  const mins = Math.max(0, Math.floor((new Date(group.next_spawn).getTime() - now) / 60000));
  const isImminent = mins <= 10;
  const isSoon = mins <= 120;

  const bandClass = isImminent
    ? "border-l-4 border-amber-300 bg-amber-400/15"
    : isSoon
      ? "border-l-4 border-amber-300/60 bg-amber-400/5"
      : "border-l-4 border-white/10";

  const hasAchievements = group.achievements.length > 0;
  const remaining = group.achievements.filter((a) => !a.done).length;
  const done = group.achievements.length - remaining;

  return (
    <section className={`${bandClass} border-b border-white/10`}>
      <header className="px-3 py-1.5 flex items-center justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs">
            <span className={`font-semibold ${isImminent ? "text-amber-300" : ""}`}>
              {group.boss_name}
            </span>
            <span className="opacity-40">·</span>
            <span className="opacity-70 truncate">📍 {group.boss_map}</span>
          </div>
          <div className="flex items-center gap-2 text-[10px] mt-0.5">
            <span className={`font-mono ${isImminent ? "text-amber-300" : "opacity-80"}`}>
              ⏰ in {formatCountdown(group.next_spawn, now)}
            </span>
            <WaypointButton code={group.waypoint_code} />
            {hasAchievements && (
              <span className="opacity-60">
                {done}/{group.achievements.length} done
              </span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {hasAchievements && (
            <button
              type="button"
              onClick={() => setOpen(!open)}
              className="text-xs opacity-60 hover:opacity-100"
              title={open ? "Hide achievements" : "Show achievements"}
            >
              {open ? "▾" : "▸"}
            </button>
          )}
          {group.explicitly_pinned && (
            <button
              type="button"
              onClick={() => void unpinBoss(group.boss_id)}
              className="opacity-40 hover:opacity-100 text-xs"
              title="Unpin boss"
            >
              ✕
            </button>
          )}
        </div>
      </header>
      {open && hasAchievements && (
        <ul>
          {group.achievements
            .slice()
            .sort((a, b) => Number(a.done) - Number(b.done))
            .map((item) => (
              <AchievementRow key={item.id} item={item} />
            ))}
        </ul>
      )}
    </section>
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

  const standaloneSorted = useMemo(() => {
    if (!pinned) return [];
    const active = pinned.standalone.filter((p) => !p.done);
    const done = pinned.standalone.filter((p) => p.done);
    return [...active, ...done];
  }, [pinned]);

  if (!pinned || (pinned.boss_groups.length === 0 && pinned.standalone.length === 0)) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-xs opacity-60 gap-3 px-4 text-center">
        <p>No pinned items yet.</p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setView("events")}
            className="px-2 py-1 text-xs bg-white/10 hover:bg-white/20 rounded"
          >
            Browse events
          </button>
          <button
            type="button"
            onClick={() => setView("catalog")}
            className="px-2 py-1 text-xs bg-white/10 hover:bg-white/20 rounded"
          >
            Legendaries
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
    <div className="flex-1 overflow-y-auto">
      {pinned.boss_groups.map((g) => (
        <BossGroupCard key={g.boss_id} group={g} now={now} />
      ))}
      {standaloneSorted.length > 0 && (
        <>
          {pinned.boss_groups.length > 0 && (
            <h3 className="px-3 py-1 text-[10px] uppercase tracking-wider opacity-60 bg-white/5">
              Other pinned
            </h3>
          )}
          <ul>
            {standaloneSorted.map((item) => (
              <AchievementRow key={item.id} item={item} />
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
