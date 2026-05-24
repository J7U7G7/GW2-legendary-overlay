import { useEffect, useState } from "react";

import { api } from "../lib/tauri";
import { stripGw2Markup } from "../lib/format";
import type { AccountItemResult, AccountItemLocation } from "../types/gw2";

const RARITY_COLORS: Record<string, string> = {
  Junk: "text-gray-500",
  Basic: "text-white",
  Fine: "text-blue-300",
  Masterwork: "text-green-300",
  Rare: "text-yellow-300",
  Exotic: "text-orange-300",
  Ascended: "text-pink-400",
  Legendary: "text-purple-400",
};

function locationLabel(loc: AccountItemLocation): string {
  // Pretty-print 'bank' / 'materials' / 'shared_inventory' / 'character:<name>'.
  if (loc.location.startsWith("character:")) {
    return `${loc.location.slice("character:".length)}${loc.location_detail ? ` · ${loc.location_detail}` : ""}`;
  }
  const base =
    loc.location === "shared_inventory"
      ? "Shared inventory"
      : loc.location.charAt(0).toUpperCase() + loc.location.slice(1);
  return loc.location_detail ? `${base} · ${loc.location_detail}` : base;
}

function ResultRow({ item }: { item: AccountItemResult }) {
  const [expanded, setExpanded] = useState(false);
  const colorClass = item.rarity ? RARITY_COLORS[item.rarity] ?? "" : "";
  return (
    <li className="border-b border-white/5">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-1.5 text-left flex items-center justify-between hover:bg-white/5"
      >
        <span className={`text-xs ${colorClass}`} title={item.name}>
          {stripGw2Markup(item.name)}
        </span>
        <span className="flex items-center gap-2 shrink-0">
          <span className="font-mono text-[10px] opacity-70">×{item.total}</span>
          <span className="opacity-50 text-[10px]">{expanded ? "▾" : "▸"}</span>
        </span>
      </button>
      {expanded && (
        <ul className="bg-white/[0.02] text-[10px]">
          {item.locations.map((loc, i) => (
            <li
              key={i}
              className="pl-6 pr-3 py-0.5 flex items-center justify-between gap-2 border-t border-white/5"
            >
              <span className="opacity-80 truncate">{locationLabel(loc)}</span>
              <span className="font-mono opacity-60 shrink-0">×{loc.count}</span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function MyItemsView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AccountItemResult[]>([]);
  const [status, setStatus] = useState<"" | "searching" | "syncing" | "error">("");
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Debounced search
  useEffect(() => {
    const t = window.setTimeout(async () => {
      const q = query.trim();
      if (q.length === 0) {
        setResults([]);
        return;
      }
      setStatus("searching");
      setErrorMsg(null);
      try {
        const r = await api.searchAccountItems(q, 30);
        setResults(r);
        setStatus("");
      } catch (e) {
        console.warn("searchAccountItems failed:", e);
        setStatus("error");
        setErrorMsg(String(e));
      }
    }, 200);
    return () => window.clearTimeout(t);
  }, [query]);

  const onSync = async () => {
    setStatus("syncing");
    setErrorMsg(null);
    try {
      const n = await api.syncAccountItems();
      setLastSync(`${n} entries · ${new Date().toLocaleTimeString()}`);
      setStatus("");
      // Re-run search to reflect new data.
      if (query.trim()) {
        const r = await api.searchAccountItems(query.trim(), 30);
        setResults(r);
      }
    } catch (e) {
      console.warn("syncAccountItems failed:", e);
      setStatus("error");
      setErrorMsg(String(e));
    }
  };

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-white/10 flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search items in your account…"
            autoFocus
            className="flex-1 font-mono text-xs px-2 py-1.5 bg-white/5 border border-white/10 rounded focus:outline-none focus:border-[var(--accent-color)]"
          />
          <button
            type="button"
            onClick={() => void onSync()}
            disabled={status === "syncing"}
            className="px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded disabled:opacity-40"
            title="Re-scan bank, materials, shared inventory, and every character"
          >
            {status === "syncing" ? "Syncing…" : "↻ Sync"}
          </button>
        </div>
        {lastSync && <p className="text-[10px] opacity-50">Last sync: {lastSync}</p>}
        {errorMsg && <p className="text-[10px] text-red-300">{errorMsg}</p>}
      </div>
      <ul className="flex-1 overflow-y-auto">
        {results.length === 0 && query.trim().length === 0 && (
          <li className="px-3 py-2 text-xs opacity-50">
            Type at least one letter to search across bank, materials, shared
            inventory, and characters. Run a sync first if you've never indexed.
          </li>
        )}
        {results.length === 0 && query.trim().length > 0 && status !== "searching" && (
          <li className="px-3 py-2 text-xs opacity-50 italic">No matches.</li>
        )}
        {results.map((item) => (
          <ResultRow key={item.item_id} item={item} />
        ))}
      </ul>
    </div>
  );
}
