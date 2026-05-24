import { useState } from "react";

import { useAppStore } from "../store/app";

export function ApiKeySetup() {
  const setApiKey = useAppStore((s) => s.setApiKey);
  const status = useAppStore((s) => s.status);
  const errorMessage = useAppStore((s) => s.errorMessage);
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);

  const [draft, setDraft] = useState("");
  const isChecking = status === "checking";
  const missing = apiKeyStatus?.missing ?? [];

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = draft.trim();
    if (!trimmed) return;
    await setApiKey(trimmed);
    setDraft("");
  };

  return (
    <div className="flex flex-col h-full px-4 py-3 gap-3">
      <div>
        <h2 className="text-sm font-semibold mb-1">GW2 API key</h2>
        <p className="text-xs opacity-70 leading-relaxed">
          Paste your API key. Required permissions:{" "}
          <code className="opacity-90">account, progression, unlocks, inventories, characters, wallet</code>.
        </p>
        <a
          className="text-xs underline opacity-70 hover:opacity-100"
          href="https://account.arena.net/applications"
          target="_blank"
          rel="noreferrer"
        >
          Create a key on account.arena.net
        </a>
      </div>

      <form className="flex flex-col gap-2" onSubmit={onSubmit}>
        <input
          type="password"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="XXXX-XXXX-XXXX-XXXX-XXXX..."
          autoComplete="off"
          spellCheck={false}
          className="font-mono text-xs px-2 py-1.5 bg-white/5 border border-white/10 rounded focus:outline-none focus:border-[var(--accent-color)]"
        />
        <button
          type="submit"
          disabled={isChecking || draft.trim().length === 0}
          className="self-end px-3 py-1.5 text-xs bg-[var(--accent-color)] text-black font-medium rounded disabled:opacity-40 disabled:cursor-not-allowed hover:brightness-110"
        >
          {isChecking ? "Checking…" : "Save"}
        </button>
      </form>

      {missing.length > 0 && (
        <div className="text-xs text-amber-300 border border-amber-400/30 bg-amber-400/10 rounded px-2 py-1.5">
          Missing permissions: {missing.join(", ")}
        </div>
      )}

      {errorMessage && status === "error" && (
        <div className="text-xs text-red-300 border border-red-400/30 bg-red-400/10 rounded px-2 py-1.5">
          {errorMessage}
        </div>
      )}
    </div>
  );
}
