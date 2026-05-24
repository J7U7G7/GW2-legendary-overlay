import { useEffect, useState } from "react";

import { useAppStore } from "../store/app";
import type { LegendaryCollection } from "../types/gw2";

function CollectionCard({ collection }: { collection: LegendaryCollection }) {
  const [open, setOpen] = useState(false);
  const pin = useAppStore((s) => s.pin);
  const unpin = useAppStore((s) => s.unpin);

  const total = collection.members.length;
  return (
    <div className="border-b border-white/5">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full px-3 py-2 text-left flex items-center justify-between hover:bg-white/5"
      >
        <div className="flex flex-col">
          <span className="text-xs font-semibold">{collection.name}</span>
          <span className="text-[10px] opacity-50">
            {collection.generation} · {collection.kind}
          </span>
        </div>
        <div className="text-[10px] font-mono opacity-60 flex items-center gap-2">
          <span>
            {collection.done_count}/{total} done
          </span>
          <span>·</span>
          <span>{collection.pinned_count} pinned</span>
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
              <button
                type="button"
                onClick={() =>
                  m.pinned ? void unpin(m.achievement_id) : void pin(m.achievement_id, collection.key)
                }
                className={
                  m.pinned
                    ? "px-1.5 py-0.5 text-[10px] bg-[var(--accent-color)] text-black rounded"
                    : "px-1.5 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
                }
              >
                {m.pinned ? "✓" : "+"}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function CatalogView() {
  const collections = useAppStore((s) => s.collections);
  const setView = useAppStore((s) => s.setView);

  // Ensure catalog is loaded when this view mounts
  useEffect(() => {
    if (collections.length === 0) {
      setView("catalog");
    }
  }, [collections.length, setView]);

  return (
    <div className="flex-1 overflow-y-auto">
      {collections.length === 0 ? (
        <div className="px-3 py-2 text-xs opacity-50 italic">No catalog entries.</div>
      ) : (
        collections.map((c) => <CollectionCard key={c.key} collection={c} />)
      )}
    </div>
  );
}
