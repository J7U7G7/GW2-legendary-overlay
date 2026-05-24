import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { useHotkeys, HOTKEY_LABELS } from "../hooks/useHotkeys";
import { useAppStore, type ViewKey } from "../store/app";
import { ApiKeySetup } from "./ApiKeySetup";
import { CatalogView } from "./CatalogView";
import { EventsTab } from "./EventsTab";
import { PinnedPanel } from "./PinnedPanel";
import { SearchView } from "./SearchView";
import { WizardsVaultPanel } from "./WizardsVaultPanel";

type TabConfig = { id: ViewKey; label: string };
const TABS: TabConfig[] = [
  { id: "pinned", label: "Pinned" },
  { id: "events", label: "Events" },
  { id: "catalog", label: "Catalog" },
  { id: "search", label: "Search" },
  { id: "wv", label: "WV" },
];

export function Overlay() {
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);
  const status = useAppStore((s) => s.status);
  const view = useAppStore((s) => s.view);
  const wv = useAppStore((s) => s.wizardsVault);
  const summary = useAppStore((s) => s.summary);
  const pinned = useAppStore((s) => s.pinned);
  const checkApiKey = useAppStore((s) => s.checkApiKey);
  const triggerSync = useAppStore((s) => s.triggerSync);
  const clearApiKey = useAppStore((s) => s.clearApiKey);
  const setView = useAppStore((s) => s.setView);
  const pinnedCount =
    (pinned?.boss_groups.length ?? 0) + (pinned?.standalone.length ?? 0);

  useEffect(() => {
    void checkApiKey();
  }, [checkApiKey]);

  useHotkeys();

  const hasUsableKey = apiKeyStatus !== null && apiKeyStatus.permissions_ok;

  return (
    <main
      className="h-screen w-screen flex flex-col text-[var(--text-color)] overflow-hidden"
      style={{ backgroundColor: "rgba(0, 0, 0, var(--bg-opacity))" }}
    >
      <Header
        canSync={hasUsableKey}
        isSyncing={status === "syncing"}
        hasKey={apiKeyStatus !== null}
        onSync={() => void triggerSync()}
        onClearKey={() => void clearApiKey()}
      />

      {!hasUsableKey ? (
        <ApiKeySetup />
      ) : (
        <>
          <Tabs current={view} onSelect={setView} pinnedCount={pinnedCount} />
          <div className="flex-1 flex flex-col overflow-hidden">
            {view === "pinned" && <PinnedPanel />}
            {view === "events" && <EventsTab />}
            {view === "catalog" && <CatalogView />}
            {view === "search" && <SearchView />}
            {view === "wv" && (
              <div className="flex-1 overflow-y-auto py-1">
                <WizardsVaultPanel label="Daily" period={wv?.daily ?? null} />
                <WizardsVaultPanel label="Weekly" period={wv?.weekly ?? null} />
                <WizardsVaultPanel label="Special" period={wv?.special ?? null} />
              </div>
            )}
          </div>
          <footer className="px-3 py-1 text-[10px] opacity-50 border-t border-white/10 font-mono shrink-0 flex items-center justify-between gap-3">
            {summary ? (
              <span>
                {summary.account_done}/{summary.account_tracked} done · {summary.points_earned} AP
              </span>
            ) : (
              <span />
            )}
            <span title="Hotkeys" className="opacity-70">
              {HOTKEY_LABELS.toggleVisibility} show/hide · {HOTKEY_LABELS.toggleClickThrough}{" "}
              click-through
            </span>
          </footer>
        </>
      )}
    </main>
  );
}

function Header(props: {
  canSync: boolean;
  isSyncing: boolean;
  hasKey: boolean;
  onSync: () => void;
  onClearKey: () => void;
}) {
  // Use the JS API explicitly because Tauri 2's data-tauri-drag-region
  // attribute injection didn't trigger drag on this user's environment.
  const onMouseDown = (e: React.MouseEvent) => {
    if (e.buttons === 1 && (e.target as HTMLElement).closest("[data-drag]")) {
      e.preventDefault();
      void getCurrentWindow().startDragging();
    }
  };

  return (
    <header
      className="flex items-center justify-between border-b border-white/10 shrink-0"
      onMouseDown={onMouseDown}
    >
      <div
        data-drag="1"
        data-tauri-drag-region
        className="flex-1 px-3 py-1.5 text-xs font-semibold cursor-grab active:cursor-grabbing"
      >
        GW2 Overlay
      </div>
      <div className="flex items-center gap-1 px-2">
        {props.canSync && (
          <button
            type="button"
            onClick={props.onSync}
            disabled={props.isSyncing}
            className="px-2 py-0.5 text-xs opacity-70 hover:opacity-100 disabled:opacity-30"
            title="Sync now"
          >
            {props.isSyncing ? (
              <span className="inline-block animate-spin">⟳</span>
            ) : (
              <>↻ Sync</>
            )}
          </button>
        )}
        {props.hasKey && (
          <button
            type="button"
            onClick={props.onClearKey}
            className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
            title="Clear API key"
          >
            ⏏
          </button>
        )}
      </div>
    </header>
  );
}

function Tabs({
  current,
  onSelect,
  pinnedCount,
}: {
  current: ViewKey;
  onSelect: (v: ViewKey) => void;
  pinnedCount: number;
}) {
  return (
    <nav className="flex border-b border-white/10 text-xs shrink-0">
      {TABS.map((t) => {
        const isActive = current === t.id;
        return (
          <button
            key={t.id}
            type="button"
            onClick={() => onSelect(t.id)}
            className={`px-3 py-1.5 border-b-2 transition-colors ${
              isActive
                ? "border-[var(--accent-color)] text-[var(--text-color)]"
                : "border-transparent opacity-60 hover:opacity-100"
            }`}
          >
            {t.label}
            {t.id === "pinned" && pinnedCount > 0 && (
              <span className="ml-1 text-[10px] opacity-70">({pinnedCount})</span>
            )}
          </button>
        );
      })}
    </nav>
  );
}
