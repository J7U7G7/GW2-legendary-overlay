import { useEffect, useState } from "react";

import { api } from "../lib/tauri";
import { stripGw2Markup } from "../lib/format";
import type {
  AccountCurrencyResult,
  AccountItemLocation,
  AccountItemResult,
} from "../types/gw2";

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
  if (loc.location.startsWith("character:")) {
    return `${loc.location.slice("character:".length)}${loc.location_detail ? ` · ${loc.location_detail}` : ""}`;
  }
  const base =
    loc.location === "shared_inventory"
      ? "Shared inventory"
      : loc.location.charAt(0).toUpperCase() + loc.location.slice(1);
  return loc.location_detail ? `${base} · ${loc.location_detail}` : base;
}

/** Gold display: a coin value (currency_id == 1) is stored in copper. Format
 * as "X g Y s Z c" only when the value exceeds 100 copper; otherwise plain
 * integer (most other currencies are integer counts). */
function formatCurrencyValue(currencyId: number, value: number): string {
  if (currencyId === 1) {
    const sign = value < 0 ? "-" : "";
    const abs = Math.abs(value);
    const g = Math.floor(abs / 10000);
    const s = Math.floor((abs % 10000) / 100);
    const c = abs % 100;
    return `${sign}${g}g ${s}s ${c}c`;
  }
  return value.toLocaleString();
}

function ItemRow({ item }: { item: AccountItemResult }) {
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

function CurrencyRow({ currency }: { currency: AccountCurrencyResult }) {
  return (
    <li className="px-3 py-1.5 flex items-center justify-between border-b border-white/5 bg-amber-400/[0.04]">
      <span className="text-xs flex items-center gap-2" title={currency.description ?? undefined}>
        <span className="opacity-50">💰</span>
        {stripGw2Markup(currency.name)}
      </span>
      <span className="font-mono text-[10px] text-amber-200">
        {formatCurrencyValue(currency.currency_id, currency.value)}
      </span>
    </li>
  );
}

export function MyItemsView() {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<AccountItemResult[]>([]);
  const [currencies, setCurrencies] = useState<AccountCurrencyResult[]>([]);
  const [status, setStatus] = useState<"" | "searching" | "syncing" | "error">("");
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const t = window.setTimeout(async () => {
      const q = query.trim();
      if (q.length === 0) {
        setItems([]);
        setCurrencies([]);
        return;
      }
      setStatus("searching");
      setErrorMsg(null);
      try {
        const [it, cu] = await Promise.all([
          api.searchAccountItems(q, 30),
          api.searchCurrencies(q, 15),
        ]);
        setItems(it);
        setCurrencies(cu);
        setStatus("");
      } catch (e) {
        console.warn("search failed:", e);
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
      const [n] = await Promise.all([
        api.syncAccountItems(),
        api.syncWallet(),
      ]);
      setLastSync(`${n} entries · ${new Date().toLocaleTimeString()}`);
      setStatus("");
      const q = query.trim();
      if (q) {
        const [it, cu] = await Promise.all([
          api.searchAccountItems(q, 30),
          api.searchCurrencies(q, 15),
        ]);
        setItems(it);
        setCurrencies(cu);
      }
    } catch (e) {
      console.warn("sync failed:", e);
      setStatus("error");
      setErrorMsg(String(e));
    }
  };

  const hasQuery = query.trim().length > 0;
  const hasResults = items.length > 0 || currencies.length > 0;

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-white/10 flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search items + currencies…"
            autoFocus
            className="flex-1 font-mono text-xs px-2 py-1.5 bg-white/5 border border-white/10 rounded focus:outline-none focus:border-[var(--accent-color)]"
          />
          <button
            type="button"
            onClick={() => void onSync()}
            disabled={status === "syncing"}
            className="px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded disabled:opacity-40"
            title="Re-scan bank, materials, shared inventory, characters, and wallet"
          >
            {status === "syncing" ? "Syncing…" : "↻ Sync"}
          </button>
        </div>
        {lastSync && <p className="text-[10px] opacity-50">Last sync: {lastSync}</p>}
        {errorMsg && <p className="text-[10px] text-red-300">{errorMsg}</p>}
      </div>
      <ul className="flex-1 overflow-y-auto">
        {!hasQuery && (
          <li className="px-3 py-2 text-xs opacity-50">
            Type at least one letter to search across bank, materials, shared
            inventory, characters, and the wallet. Run a sync first if you've
            never indexed.
          </li>
        )}
        {hasQuery && !hasResults && status !== "searching" && (
          <li className="px-3 py-2 text-xs opacity-50 italic">No matches.</li>
        )}
        {currencies.map((c) => (
          <CurrencyRow key={c.currency_id} currency={c} />
        ))}
        {items.map((item) => (
          <ItemRow key={item.item_id} item={item} />
        ))}
      </ul>
    </div>
  );
}
