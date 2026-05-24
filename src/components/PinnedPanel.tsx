import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import { eventTimeLabel, fillRequirement, stripGw2Markup } from "../lib/format";
import type { PinnedBit, PinnedBossGroup, PinnedItem } from "../types/gw2";

function searchWikiUrl(query: string) {
  return `https://wiki.guildwars2.com/wiki/Special:Search?search=${encodeURIComponent(query)}`;
}

function BitRow({ bit, urgent }: { bit: PinnedBit; urgent: boolean }) {
  const cleanText = stripGw2Markup(bit.text);
  const cleanName = stripGw2Markup(bit.resolved_name);
  const cleanDesc = stripGw2Markup(bit.resolved_description);
  const hasText = cleanText.length > 0;
  const primaryLabel =
    cleanName ||
    (hasText
      ? cleanText
      : bit.ref_id !== null
        ? `${bit.kind} #${bit.ref_id}`
        : bit.kind);
  // Sub-line: only show things that add information *over* the primary label.
  const seen = new Set<string>([primaryLabel]);
  const subLines: string[] = [];
  for (const candidate of [cleanText, cleanDesc]) {
    if (candidate.length > 0 && !seen.has(candidate)) {
      seen.add(candidate);
      subLines.push(candidate);
    }
  }
  const wikiQuery = bit.resolved_name
    ? bit.resolved_name
    : bit.ref_id !== null && bit.kind !== "Text"
      ? `${bit.kind}:${bit.ref_id}`
      : hasText
        ? bit.text
        : null;
  const rowClass = bit.done
    ? "opacity-40"
    : urgent
      ? "bg-amber-400/10 border-l-2 border-amber-300"
      : "opacity-90 border-l-2 border-transparent";
  return (
    <li className={`pl-9 pr-3 py-0.5 flex items-start gap-1.5 text-[10px] ${rowClass}`}>
      <span
        className={
          bit.done
            ? "text-[var(--accent-color)] mt-0.5"
            : urgent
              ? "text-amber-300 mt-0.5"
              : "opacity-50 mt-0.5"
        }
      >
        {bit.done ? "✓" : "○"}
      </span>
      <div className="flex-1 leading-tight min-w-0">
        <div className={bit.done ? "line-through" : ""}>{primaryLabel}</div>
        {!bit.done &&
          subLines.map((line, i) => (
            <div key={i} className="opacity-60 text-[10px] mt-px">
              {line}
            </div>
          ))}
      </div>
      {wikiQuery && (
        <a
          className="opacity-50 hover:opacity-100 text-[10px] mt-0.5"
          href={searchWikiUrl(wikiQuery)}
          target="_blank"
          rel="noreferrer"
          title="Open wiki"
        >
          🔗
        </a>
      )}
    </li>
  );
}

function AchievementDetails({ item }: { item: PinnedItem }) {
  const desc = stripGw2Markup(item.description);
  const req = fillRequirement(stripGw2Markup(item.requirement), item.max);
  return (
    <div className="pl-6 pr-3 pb-1.5 pt-0.5 text-[10px] opacity-90 leading-snug">
      {desc && <p className="opacity-80 mb-1 whitespace-pre-line">{desc}</p>}
      {req && <p className="opacity-70 italic mb-1 whitespace-pre-line">{req}</p>}
      <a
        className="text-[var(--accent-color)] opacity-80 hover:opacity-100 underline text-[10px]"
        href={searchWikiUrl(item.name)}
        target="_blank"
        rel="noreferrer"
      >
        Open achievement on wiki ↗
      </a>
    </div>
  );
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

function WaypointButton({ code, name }: { code: string | null; name?: string }) {
  const [copied, setCopied] = useState(false);
  if (!code) return null;
  const text = name ? `${name} ${code}` : code;
  const onClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(text);
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
      title={`Copy '${text}' to clipboard`}
    >
      {copied ? "✓ copied" : "📋 WP"}
    </button>
  );
}

function AchievementRow({
  item,
  bossImminent = false,
}: {
  item: PinnedItem;
  bossImminent?: boolean;
}) {
  const unpin = useAppStore((s) => s.unpin);
  const pin = useAppStore((s) => s.pin);
  const [open, setOpen] = useState(false);
  const ratio = item.completion_ratio;
  const ratioLabel =
    item.current !== null && item.max !== null && item.max > 0
      ? `${item.current}/${item.max}`
      : item.done
        ? "done"
        : "0%";
  const hasDetails = !!(item.description || item.requirement || item.bits.length > 0);

  const rowOpacity = item.is_pinned ? "" : "opacity-60";
  return (
    <li className={`border-t border-white/5 ${rowOpacity}`}>
      <div className="pl-6 pr-3 py-1 flex items-center justify-between gap-2 text-[11px]">
        <button
          type="button"
          onClick={() => hasDetails && setOpen(!open)}
          disabled={!hasDetails}
          className="flex-1 min-w-0 text-left disabled:cursor-default"
          title={hasDetails ? (open ? "Hide details" : "Show details") : undefined}
        >
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
            {hasDetails && (
              <span className="opacity-40 text-[10px] ml-auto">{open ? "▾" : "▸"}</span>
            )}
          </div>
          {!item.done && item.max !== null && item.max > 0 && (
            <div className="ml-5 mt-0.5">
              <ProgressBar ratio={ratio} />
            </div>
          )}
        </button>
        <div className="flex items-center gap-2 shrink-0">
          <span className="font-mono text-[10px] opacity-60">{ratioLabel}</span>
          {item.is_pinned ? (
            <button
              type="button"
              onClick={() => void unpin(item.id)}
              className="opacity-40 hover:opacity-100 text-xs"
              title="Unpin"
            >
              ✕
            </button>
          ) : (
            <button
              type="button"
              onClick={() => void pin(item.id, null)}
              className="opacity-40 hover:opacity-100 text-xs"
              title="Pin this achievement"
            >
              📌
            </button>
          )}
        </div>
      </div>
      {open && hasDetails && (
        <>
          <AchievementDetails item={item} />
          {item.bits.length > 0 && (
            <ul className="pb-1">
              {item.bits.map((bit) => (
                <BitRow key={bit.index} bit={bit} urgent={bossImminent && !bit.done} />
              ))}
            </ul>
          )}
        </>
      )}
    </li>
  );
}

function BossGroupCard({ group, now }: { group: PinnedBossGroup; now: number }) {
  // Default open whenever the group has any achievements — the user wants to
  // see both done and to-do at a glance.
  const [open, setOpen] = useState(group.achievements.length > 0);
  const removeBossGroup = useAppStore((s) => s.removeBossGroup);
  const removeTitle =
    group.explicitly_pinned && group.achievements.length > 0
      ? `Unpin ${group.boss_name} and ${group.achievements.length} linked achievement(s)`
      : group.explicitly_pinned
        ? `Unpin ${group.boss_name}`
        : `Unpin ${group.achievements.length} linked achievement(s)`;
  const time = eventTimeLabel(group.next_spawn, group.duration_minutes, now);

  const isActive = time.status === "active";
  const isImminent = time.status === "future" && time.minutesUntilStart <= 10;
  const isSoon = time.status === "future" && time.minutesUntilStart <= 120;

  const bandClass = isActive
    ? "border-l-4 border-green-400 bg-green-400/15"
    : isImminent
      ? "border-l-4 border-orange-500 bg-orange-500/20"
      : isSoon
        ? "border-l-4 border-amber-300/60 bg-amber-300/5"
        : "border-l-4 border-white/10";

  const hasAchievements = group.achievements.length > 0;
  const remaining = group.achievements.filter((a) => !a.done).length;
  const done = group.achievements.length - remaining;

  return (
    <section className={`${bandClass} border-b border-white/10`}>
      <header className="px-3 py-1.5 flex items-center justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs">
            <span
              className={`font-semibold ${isActive ? "text-green-300" : isImminent ? "text-orange-300" : ""}`}
            >
              {group.boss_name}
            </span>
            <span className="opacity-40">·</span>
            <span className="opacity-70 truncate">📍 {group.boss_map}</span>
          </div>
          <div className="flex items-center gap-2 text-[10px] mt-0.5">
            <span
              className={`font-mono ${isActive ? "text-green-300 font-semibold" : isImminent ? "text-orange-300 font-semibold" : "opacity-80"}`}
            >
              ⏰ {time.label}
            </span>
            <WaypointButton code={group.waypoint_code} name={group.boss_name} />
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
          <button
            type="button"
            onClick={() => void removeBossGroup(group.boss_id)}
            className="opacity-40 hover:opacity-100 text-xs"
            title={removeTitle}
          >
            ✕
          </button>
        </div>
      </header>
      {open && hasAchievements && (
        <ul>
          {group.achievements
            .slice()
            .sort((a, b) => {
              // Pinned first, then by done status (not done before done).
              if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
              return Number(a.done) - Number(b.done);
            })
            .map((item) => (
              <AchievementRow key={item.id} item={item} bossImminent={isSoon || isImminent} />
            ))}
        </ul>
      )}
    </section>
  );
}

function useTickingNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

function EmptyState({ message, suggestion }: { message: string; suggestion?: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-xs opacity-60 gap-2 px-4 text-center">
      <p>{message}</p>
      {suggestion && <p className="text-[10px] opacity-70">{suggestion}</p>}
    </div>
  );
}

/** Show only pinned world boss / meta event groups. */
export function BossesView() {
  const pinned = useAppStore((s) => s.pinned);
  const now = useTickingNow();
  if (!pinned || pinned.boss_groups.length === 0) {
    return (
      <EmptyState
        message="No pinned bosses yet."
        suggestion="Open the main overlay → Events tab to pin world bosses and meta events."
      />
    );
  }
  return (
    <div className="flex-1 overflow-y-auto">
      {pinned.boss_groups.map((g) => (
        <BossGroupCard key={g.boss_id} group={g} now={now} />
      ))}
    </div>
  );
}

/** Show only standalone (boss-less) pins: legendary steps, raid achievements, ad-hoc pins. */
export function AchievementsView() {
  const pinned = useAppStore((s) => s.pinned);
  const standaloneSorted = useMemo(() => {
    if (!pinned) return [];
    const active = pinned.standalone.filter((p) => !p.done);
    const done = pinned.standalone.filter((p) => p.done);
    return [...active, ...done];
  }, [pinned]);
  if (standaloneSorted.length === 0) {
    return (
      <EmptyState
        message="No pinned achievements yet."
        suggestion="Open the main overlay → Catalog or Search to pin collection steps."
      />
    );
  }
  return (
    <div className="flex-1 overflow-y-auto">
      <ul>
        {standaloneSorted.map((item) => (
          <AchievementRow key={item.id} item={item} />
        ))}
      </ul>
    </div>
  );
}
