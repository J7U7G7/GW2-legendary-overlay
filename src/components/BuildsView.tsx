import { useEffect, useMemo, useState } from "react";

import { api } from "../lib/tauri";
import type { Build } from "../types/gw2";

function BuildCard({ build }: { build: Build }) {
  const [copied, setCopied] = useState(false);
  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(build.chat_code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.warn("copy failed:", e);
    }
  };
  return (
    <article className="border-b border-white/5 px-3 py-2">
      <div className="flex items-center justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-xs font-semibold truncate" title={build.name}>
            {build.name}
          </div>
          <div className="text-[10px] opacity-70 truncate">
            {build.role}
            {build.elite_spec ? ` · ${build.elite_spec}` : ""}
            {build.weapons ? ` · ${build.weapons}` : ""}
            {build.difficulty != null
              ? ` · ${"⭐".repeat(build.difficulty)}${"☆".repeat(Math.max(0, 3 - build.difficulty))}`
              : ""}
          </div>
          {build.gear_summary && (
            <div className="text-[10px] opacity-60 truncate">⚔ {build.gear_summary}</div>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <button
            type="button"
            onClick={() => void onCopy()}
            className={
              copied
                ? "px-2 py-0.5 text-[10px] bg-[var(--accent-color)] text-black rounded"
                : "px-2 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
            }
            title={`Copy: ${build.chat_code}`}
          >
            {copied ? "✓ copied" : "📋 Code"}
          </button>
          <a
            href={build.source_url}
            target="_blank"
            rel="noreferrer"
            className="px-2 py-0.5 text-[10px] opacity-60 hover:opacity-100"
            title={`Open on ${build.source}`}
          >
            ↗
          </a>
        </div>
      </div>
      {build.notes && (
        <p className="text-[10px] opacity-50 italic mt-1">{build.notes}</p>
      )}
    </article>
  );
}

export function BuildsView() {
  const [all, setAll] = useState<Build[]>([]);
  const [profession, setProfession] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api
      .listBuilds()
      .then(setAll)
      .catch((e) => setError(String(e)));
  }, []);

  const professions = useMemo(() => {
    const set = new Set<string>();
    for (const b of all) set.add(b.profession);
    return Array.from(set).sort();
  }, [all]);

  const filtered = useMemo(
    () => (profession ? all.filter((b) => b.profession === profession) : all),
    [all, profession],
  );

  if (error) {
    return <div className="px-3 py-2 text-xs text-red-300">{error}</div>;
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="px-3 py-1.5 border-b border-white/10 flex items-center gap-1 flex-wrap text-[10px] shrink-0">
        <button
          type="button"
          onClick={() => setProfession(null)}
          className={
            profession === null
              ? "px-1.5 py-0.5 rounded bg-[var(--accent-color)] text-black"
              : "px-1.5 py-0.5 rounded bg-white/10 hover:bg-white/20"
          }
        >
          All
        </button>
        {professions.map((p) => (
          <button
            key={p}
            type="button"
            onClick={() => setProfession(p)}
            className={
              profession === p
                ? "px-1.5 py-0.5 rounded bg-[var(--accent-color)] text-black"
                : "px-1.5 py-0.5 rounded bg-white/10 hover:bg-white/20"
            }
          >
            {p}
          </button>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto">
        {filtered.length === 0 ? (
          <div className="px-3 py-2 text-xs opacity-60 italic">No builds for this profession.</div>
        ) : (
          filtered.map((b) => <BuildCard key={b.id} build={b} />)
        )}
        <p className="px-3 py-2 text-[10px] opacity-50 italic">
          Builds are a curated subset. To add more, drop entries in
          <code> src-tauri/data/builds.json</code> with their chat codes from in-game (Hero panel →
          Build template → right-click → Copy Chat Code).
        </p>
      </div>
    </div>
  );
}
