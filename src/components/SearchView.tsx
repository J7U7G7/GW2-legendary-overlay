import { useEffect } from "react";

import { useAppStore } from "../store/app";

export function SearchView() {
  const query = useAppStore((s) => s.searchQuery);
  const setQuery = useAppStore((s) => s.setSearchQuery);
  const runSearch = useAppStore((s) => s.runSearch);
  const results = useAppStore((s) => s.searchResults);
  const pin = useAppStore((s) => s.pin);

  // Debounced search
  useEffect(() => {
    const id = window.setTimeout(() => void runSearch(), 200);
    return () => window.clearTimeout(id);
  }, [query, runSearch]);

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-white/10">
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search achievements (name)…"
          autoFocus
          className="w-full font-mono text-xs px-2 py-1.5 bg-white/5 border border-white/10 rounded focus:outline-none focus:border-[var(--accent-color)]"
        />
      </div>
      <ul className="flex-1 overflow-y-auto text-xs">
        {results.length === 0 && query.trim().length > 0 && (
          <li className="px-3 py-2 opacity-50 italic">No matches in cache.</li>
        )}
        {results.length === 0 && query.trim().length === 0 && (
          <li className="px-3 py-2 opacity-50">Type at least one letter.</li>
        )}
        {results.map((r) => (
          <li
            key={r.id}
            className="px-3 py-1.5 border-b border-white/5 flex items-center justify-between gap-2"
          >
            <div className="flex-1 min-w-0">
              <div className="truncate" title={r.name}>
                {r.name}
              </div>
              {r.description && (
                <div className="text-[10px] opacity-50 truncate" title={r.description}>
                  {r.description}
                </div>
              )}
            </div>
            {r.pinned ? (
              <span
                className="px-2 py-0.5 text-[10px] text-[var(--accent-color)] cursor-default"
                title="Already pinned. Unpin from the Achievements window."
              >
                ✓ Pinned
              </span>
            ) : (
              <button
                type="button"
                onClick={() => void pin(r.id, null)}
                className="px-2 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
              >
                + Pin
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
