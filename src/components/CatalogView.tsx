import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import { wikiUrl } from "../lib/format";
import { api } from "../lib/tauri";
import type { LegendaryCollection, LegendaryProgress, MissingLeaf } from "../types/gw2";

const KIND_LABEL: Record<string, string> = {
  weapon: "Weapon",
  armor: "Armor",
  trinket: "Trinket",
  backpack: "Back",
};

const EXPANSION_ORDER = ["Core", "HoT", "PoF", "LWS3", "LWS4", "EoD", "SotO", "JW"];

function KindBadge({ kind }: { kind: string }) {
  const label = KIND_LABEL[kind] ?? kind;
  return (
    <span className="text-[9px] uppercase tracking-wide px-1 py-px rounded bg-white/10 opacity-70">
      {label}
    </span>
  );
}

function formatLeafQty(leaf: MissingLeaf): string {
  if (leaf.kind === "currency" && leaf.id === 1) {
    // Coin in copper. Format as gold/silver for missing > a gold's worth.
    const abs = Math.abs(leaf.missing);
    if (abs >= 100) {
      const g = Math.floor(abs / 10000);
      const s = Math.floor((abs % 10000) / 100);
      const c = abs % 100;
      return `${g}g ${s}s ${c}c`;
    }
  }
  return leaf.missing.toLocaleString();
}

function ProgressBar({ ratio }: { ratio: number }) {
  const pct = Math.max(0, Math.min(1, ratio)) * 100;
  return (
    <div className="h-1 bg-white/10 rounded overflow-hidden">
      <div
        className="h-full bg-[var(--accent-color)] transition-all"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

function RecipeProgressBlock({ progress }: { progress: LegendaryProgress }) {
  const pct = Math.round(progress.ratio * 100);
  return (
    <div className="border-t border-white/5 px-3 py-2 bg-white/[0.03]">
      <div className="flex items-center justify-between text-[10px] mb-1.5">
        <span className="opacity-70 font-semibold">
          📦 Recipe: {pct}% complete
        </span>
        <span className="opacity-60 font-mono">
          {progress.leaves_complete}/{progress.leaves_total} leaves
        </span>
      </div>
      <ProgressBar ratio={progress.ratio} />
      {progress.top_missing.length === 0 ? (
        <p className="text-[10px] text-[var(--accent-color)] italic mt-1.5">
          All tracked leaves complete. Remaining work is in achievement steps.
        </p>
      ) : (
        <ul className="mt-1.5 space-y-0.5">
          {progress.top_missing.map((leaf, i) => (
            <li
              key={`${leaf.kind}:${leaf.id}:${i}`}
              className="text-[10px] flex items-center justify-between gap-2"
              title={leaf.name}
            >
              <span className="truncate opacity-80">
                {leaf.kind === "currency" ? "💰 " : ""}
                {leaf.name}
              </span>
              <span className="font-mono shrink-0">
                <span className="opacity-50">{leaf.owned.toLocaleString()}</span>
                <span className="opacity-50">/</span>
                <span>{leaf.needed.toLocaleString()}</span>
                <span className="text-red-300/80 ml-1">−{formatLeafQty(leaf)}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function CollectionCard({
  collection,
  progress,
}: {
  collection: LegendaryCollection;
  progress: LegendaryProgress | undefined;
}) {
  const [open, setOpen] = useState(false);
  const pin = useAppStore((s) => s.pin);

  const total = collection.members.length;
  return (
    <div className="border-b border-white/5">
      <div className="w-full px-3 py-2 flex items-center justify-between hover:bg-white/5">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="flex-1 min-w-0 text-left flex flex-col"
        >
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-semibold truncate">{collection.name}</span>
            <KindBadge kind={collection.kind} />
            {progress && progress.ratio >= 0.95 && (
              <span
                className="text-[9px] px-1 py-px rounded bg-[var(--accent-color)] text-black"
                title="Recipe leaves nearly complete"
              >
                READY?
              </span>
            )}
          </div>
          <span className="text-[10px] opacity-50">
            {collection.generation}
            {progress != null && ` · 📦 ${Math.round(progress.ratio * 100)}%`}
          </span>
        </button>
        <div className="text-[10px] font-mono opacity-60 flex items-center gap-2 shrink-0">
          <span>
            {collection.done_count}/{total} done
          </span>
          {collection.pinned_count > 0 && (
            <>
              <span>·</span>
              <span className="text-[var(--accent-color)]">{collection.pinned_count} pinned</span>
            </>
          )}
          <a
            href={wikiUrl(collection.name)}
            target="_blank"
            rel="noreferrer"
            className="opacity-60 hover:opacity-100"
            title={`Recipe on wiki: ${collection.name}`}
          >
            📖
          </a>
          <button
            type="button"
            onClick={() => setOpen(!open)}
            className="opacity-60 hover:opacity-100"
          >
            {open ? "▾" : "▸"}
          </button>
        </div>
      </div>
      {open && (
        <>
          {progress && <RecipeProgressBlock progress={progress} />}
          <ul className="bg-white/[0.02]">
            {collection.members.map((m) => (
              <li
                key={m.achievement_id}
                className="px-3 py-1 text-xs flex items-center justify-between gap-2 border-t border-white/5"
              >
                <div className="flex-1 min-w-0 flex items-center gap-1.5">
                  <span
                    className={
                      m.done
                        ? "text-[var(--accent-color)]"
                        : m.completion_ratio > 0
                          ? "text-amber-300"
                          : "opacity-40"
                    }
                  >
                    {m.done ? "✓" : m.completion_ratio > 0 ? "◐" : "○"}
                  </span>
                  <span className={m.done ? "opacity-50 line-through truncate" : "truncate"}>
                    {m.name}
                  </span>
                </div>
                {m.pinned ? (
                  <span
                    className="px-1.5 py-0.5 text-[10px] text-[var(--accent-color)] cursor-default"
                    title="Already pinned. Unpin from the Achievements window."
                  >
                    ✓ Pinned
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => void pin(m.achievement_id, collection.key)}
                    className="px-1.5 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
                  >
                    + Pin
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

function expansionRank(e: string): number {
  const idx = EXPANSION_ORDER.indexOf(e);
  return idx === -1 ? 99 : idx;
}

export function CatalogView() {
  const collections = useAppStore((s) => s.collections);
  const setView = useAppStore((s) => s.setView);
  const [kindFilter, setKindFilter] = useState<string | null>(null);
  const [progressByKey, setProgressByKey] = useState<Map<string, LegendaryProgress>>(
    new Map(),
  );

  useEffect(() => {
    if (collections.length === 0) {
      setView("catalog");
    }
  }, [collections.length, setView]);

  // Recipe progress depends on inventory + wallet which are synced
  // periodically. Fetch on mount; re-fetch when the user explicitly clicks
  // the refresh button in the filter bar.
  const refreshProgress = async () => {
    try {
      const list = await api.legendaryProgress();
      const map = new Map<string, LegendaryProgress>();
      for (const p of list) map.set(p.collection_key, p);
      setProgressByKey(map);
    } catch (e) {
      console.warn("legendaryProgress failed:", e);
    }
  };

  useEffect(() => {
    void refreshProgress();
  }, []);

  const groups = useMemo(() => {
    const filtered = kindFilter
      ? collections.filter((c) => c.kind === kindFilter)
      : collections;
    const map = new Map<string, LegendaryCollection[]>();
    for (const c of filtered) {
      const arr = map.get(c.generation) ?? [];
      arr.push(c);
      map.set(c.generation, arr);
    }
    for (const arr of map.values()) {
      arr.sort((a, b) => {
        // Closest-to-complete first (when we have progress data), then
        // fall back to curated sort_order.
        const pa = progressByKey.get(a.key)?.ratio ?? -1;
        const pb = progressByKey.get(b.key)?.ratio ?? -1;
        if (pa !== pb) return pb - pa;
        return a.sort_order - b.sort_order || a.name.localeCompare(b.name);
      });
    }
    return Array.from(map.entries()).sort(
      ([a], [b]) => expansionRank(a) - expansionRank(b) || a.localeCompare(b),
    );
  }, [collections, kindFilter, progressByKey]);

  const kinds = useMemo(() => {
    const set = new Set<string>();
    for (const c of collections) set.add(c.kind);
    return Array.from(set).sort();
  }, [collections]);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="px-3 py-1.5 border-b border-white/10 flex items-center gap-1.5 text-[10px] shrink-0">
        <button
          type="button"
          onClick={() => setKindFilter(null)}
          className={
            kindFilter === null
              ? "px-1.5 py-0.5 rounded bg-[var(--accent-color)] text-black"
              : "px-1.5 py-0.5 rounded bg-white/10 hover:bg-white/20"
          }
        >
          All
        </button>
        {kinds.map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => setKindFilter(k)}
            className={
              kindFilter === k
                ? "px-1.5 py-0.5 rounded bg-[var(--accent-color)] text-black"
                : "px-1.5 py-0.5 rounded bg-white/10 hover:bg-white/20"
            }
          >
            {KIND_LABEL[k] ?? k}
          </button>
        ))}
        <button
          type="button"
          onClick={() => void refreshProgress()}
          className="ml-auto px-1.5 py-0.5 rounded bg-white/10 hover:bg-white/20"
          title="Refresh recipe progress (uses current items + wallet snapshot)"
        >
          ↻ Recipes
        </button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {groups.length === 0 ? (
          <div className="px-3 py-2 text-xs opacity-50 italic">No catalog entries.</div>
        ) : (
          groups.map(([generation, items]) => (
            <section key={generation}>
              <h3 className="sticky top-0 px-3 py-1 text-[10px] font-semibold uppercase tracking-wider bg-black/60 backdrop-blur opacity-80 border-b border-white/10">
                {generation}
              </h3>
              {items.map((c) => (
                <CollectionCard
                  key={c.key}
                  collection={c}
                  progress={progressByKey.get(c.key)}
                />
              ))}
            </section>
          ))
        )}
      </div>
    </div>
  );
}
