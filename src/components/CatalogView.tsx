import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "../store/app";
import type { LegendaryCollection } from "../types/gw2";

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

function CollectionCard({ collection }: { collection: LegendaryCollection }) {
  const [open, setOpen] = useState(false);
  const pin = useAppStore((s) => s.pin);

  const total = collection.members.length;
  return (
    <div className="border-b border-white/5">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full px-3 py-2 text-left flex items-center justify-between hover:bg-white/5"
      >
        <div className="flex flex-col min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-semibold truncate">{collection.name}</span>
            <KindBadge kind={collection.kind} />
          </div>
          <span className="text-[10px] opacity-50">{collection.generation}</span>
        </div>
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
          <span className="ml-1 opacity-60">{open ? "▾" : "▸"}</span>
        </div>
      </button>
      {open && (
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
                  title="Already pinned. Unpin from the Pinned tab."
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

  useEffect(() => {
    if (collections.length === 0) {
      setView("catalog");
    }
  }, [collections.length, setView]);

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
      arr.sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name));
    }
    return Array.from(map.entries()).sort(
      ([a], [b]) => expansionRank(a) - expansionRank(b) || a.localeCompare(b),
    );
  }, [collections, kindFilter]);

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
                <CollectionCard key={c.key} collection={c} />
              ))}
            </section>
          ))
        )}
      </div>
    </div>
  );
}
