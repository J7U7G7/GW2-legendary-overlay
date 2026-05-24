import { useEffect } from "react";

import { useAppStore } from "../store/app";
import { ApiKeySetup } from "./ApiKeySetup";
import { BossTimer } from "./BossTimer";
import { WizardsVaultPanel } from "./WizardsVaultPanel";

export function Overlay() {
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);
  const status = useAppStore((s) => s.status);
  const upcoming = useAppStore((s) => s.upcoming);
  const wv = useAppStore((s) => s.wizardsVault);
  const summary = useAppStore((s) => s.summary);
  const checkApiKey = useAppStore((s) => s.checkApiKey);
  const triggerSync = useAppStore((s) => s.triggerSync);
  const clearApiKey = useAppStore((s) => s.clearApiKey);

  useEffect(() => {
    void checkApiKey();
  }, [checkApiKey]);

  const nextEvent = upcoming[0] ?? null;
  const hasUsableKey = apiKeyStatus !== null && apiKeyStatus.permissions_ok;

  return (
    <main
      className="h-screen w-screen flex flex-col text-[var(--text-color)] overflow-hidden"
      style={{ backgroundColor: "rgba(0, 0, 0, var(--bg-opacity))" }}
    >
      <header
        data-tauri-drag-region
        className="px-3 py-1.5 text-xs font-semibold flex items-center justify-between border-b border-white/10 cursor-grab select-none"
      >
        <span data-tauri-drag-region>GW2 Overlay</span>
        <div className="flex items-center gap-2" data-tauri-drag-region={false}>
          {hasUsableKey && (
            <button
              type="button"
              onClick={triggerSync}
              disabled={status === "syncing"}
              className="opacity-60 hover:opacity-100 disabled:opacity-30"
              title="Sync now"
            >
              {status === "syncing" ? "⟳" : "↻"}
            </button>
          )}
          {apiKeyStatus && (
            <button
              type="button"
              onClick={clearApiKey}
              className="opacity-60 hover:opacity-100"
              title="Clear API key"
            >
              ⏏
            </button>
          )}
        </div>
      </header>

      {!hasUsableKey ? (
        <ApiKeySetup />
      ) : (
        <>
          <BossTimer next={nextEvent} />
          <section className="flex-1 overflow-y-auto py-1">
            <WizardsVaultPanel label="Wizard's Vault — Daily" period={wv?.daily ?? null} />
            <WizardsVaultPanel label="Wizard's Vault — Weekly" period={wv?.weekly ?? null} />
            <WizardsVaultPanel label="Wizard's Vault — Special" period={wv?.special ?? null} />
          </section>
          {summary && (
            <footer className="px-3 py-1 text-[10px] opacity-50 border-t border-white/10 font-mono">
              {summary.account_done}/{summary.account_tracked} done · {summary.points_earned} AP ·
              cache: {summary.total_achievements_in_cache}
            </footer>
          )}
        </>
      )}
    </main>
  );
}
