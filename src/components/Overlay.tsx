import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { useCollapse } from "../hooks/useCollapse";
import { useCrossWindowSync } from "../hooks/useCrossWindowSync";
import { useHotkeys, HOTKEY_LABELS } from "../hooks/useHotkeys";
import { api } from "../lib/tauri";
import { useAppStore, type ViewKey } from "../store/app";
import { useSettingsStore } from "../store/settings";
import { ApiKeySetup } from "./ApiKeySetup";
import { BuildsView } from "./BuildsView";
import { CatalogView } from "./CatalogView";
import { EventsTab } from "./EventsTab";
import { MyItemsView } from "./MyItemsView";
import { SearchView } from "./SearchView";
import { SettingsPanel } from "./SettingsPanel";
import { TodosView } from "./TodosView";
import { WizardsVaultPanel } from "./WizardsVaultPanel";

type TabConfig = { id: ViewKey; label: string };
const TABS: TabConfig[] = [
  { id: "events", label: "Events" },
  { id: "catalog", label: "Catalog" },
  { id: "search", label: "Search" },
  { id: "items", label: "Items" },
  { id: "todos", label: "Todos" },
  { id: "builds", label: "Builds" },
  { id: "wv", label: "WV" },
];

async function toggleWindowByLabel(label: string) {
  const w = await WebviewWindow.getByLabel(label);
  if (!w) return;
  if (await w.isVisible()) {
    await w.hide();
  } else {
    await w.show();
    await w.setFocus();
  }
}

export function Overlay() {
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);
  const status = useAppStore((s) => s.status);
  const view = useAppStore((s) => s.view);
  const wv = useAppStore((s) => s.wizardsVault);
  const summary = useAppStore((s) => s.summary);
  const checkApiKey = useAppStore((s) => s.checkApiKey);
  const triggerSync = useAppStore((s) => s.triggerSync);
  const clearApiKey = useAppStore((s) => s.clearApiKey);
  const setView = useAppStore((s) => s.setView);
  const loadSettings = useSettingsStore((s) => s.load);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { collapsed, toggle: toggleCollapse } = useCollapse();

  useEffect(() => {
    void checkApiKey();
    void loadSettings();
  }, [checkApiKey, loadSettings]);

  useHotkeys();
  useCrossWindowSync();

  const hasUsableKey = apiKeyStatus !== null && apiKeyStatus.permissions_ok;

  return (
    <main
      className="h-screen w-screen flex flex-col text-[var(--text-color)] overflow-hidden"
      style={{ backgroundColor: "var(--bg-color-rgba, rgba(0, 0, 0, 0.85))" }}
    >
      <Header
        canSync={hasUsableKey}
        isSyncing={status === "syncing"}
        hasKey={apiKeyStatus !== null}
        settingsOpen={settingsOpen}
        collapsed={collapsed}
        onSync={() => void triggerSync()}
        onClearKey={() => void clearApiKey()}
        onToggleSettings={() => setSettingsOpen(!settingsOpen)}
        onShowBosses={() => void toggleWindowByLabel("bosses")}
        onShowAchievements={() => void toggleWindowByLabel("achievements")}
        onToggleCollapse={toggleCollapse}
        onQuit={() => void api.saveStateAndQuit()}
      />

      {collapsed ? null : settingsOpen ? (
        <SettingsPanel onClose={() => setSettingsOpen(false)} />
      ) : !hasUsableKey ? (
        <ApiKeySetup />
      ) : (
        <>
          <Tabs current={view} onSelect={setView} />
          <div className="ui-zoom flex-1 flex flex-col overflow-hidden">
            {view === "events" && <EventsTab />}
            {view === "catalog" && <CatalogView />}
            {view === "search" && <SearchView />}
            {view === "items" && <MyItemsView />}
            {view === "todos" && <TodosView />}
            {view === "builds" && <BuildsView />}
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
              {HOTKEY_LABELS.toggleVisibility} hide · {HOTKEY_LABELS.toggleClickThrough} c-through ·{" "}
              {HOTKEY_LABELS.toggleBosses} bosses · {HOTKEY_LABELS.toggleAchievements} pinned
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
  settingsOpen: boolean;
  collapsed: boolean;
  onSync: () => void;
  onClearKey: () => void;
  onToggleSettings: () => void;
  onShowBosses: () => void;
  onShowAchievements: () => void;
  onToggleCollapse: () => void;
  onQuit: () => void;
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
      className="flex items-center justify-between border-b border-white/10 shrink-0 min-w-0"
      onMouseDown={onMouseDown}
    >
      <div
        data-drag="1"
        data-tauri-drag-region
        className="flex-1 min-w-0 px-3 py-1.5 text-xs font-semibold cursor-grab active:cursor-grabbing truncate"
      >
        GW2 Overlay
      </div>
      <div className="flex items-center gap-1 px-2 shrink-0">
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
        <button
          type="button"
          onClick={props.onShowBosses}
          className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
          title="Show pinned bosses window"
        >
          🐉
        </button>
        <button
          type="button"
          onClick={props.onShowAchievements}
          className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
          title="Show pinned achievements window"
        >
          📌
        </button>
        <button
          type="button"
          onClick={props.onToggleSettings}
          className={`px-2 py-0.5 text-xs ${props.settingsOpen ? "text-[var(--accent-color)]" : "opacity-50 hover:opacity-100"}`}
          title="Settings"
        >
          ⚙
        </button>
        <button
          type="button"
          onClick={props.onToggleCollapse}
          className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
          title={props.collapsed ? "Expand window" : "Collapse to header bar"}
        >
          {props.collapsed ? "▾" : "▴"}
        </button>
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
        <button
          type="button"
          onClick={props.onQuit}
          className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
          title="Save layout and quit"
        >
          ⏻
        </button>
      </div>
    </header>
  );
}

function Tabs({
  current,
  onSelect,
}: {
  current: ViewKey;
  onSelect: (v: ViewKey) => void;
}) {
  return (
    <nav className="flex border-b border-white/10 text-xs shrink-0 overflow-x-auto whitespace-nowrap">
      {TABS.map((t) => {
        const isActive = current === t.id;
        return (
          <button
            key={t.id}
            type="button"
            onClick={() => onSelect(t.id)}
            className={`shrink-0 px-3 py-1.5 border-b-2 transition-colors ${
              isActive
                ? "border-[var(--accent-color)] text-[var(--text-color)]"
                : "border-transparent opacity-60 hover:opacity-100"
            }`}
          >
            {t.label}
          </button>
        );
      })}
    </nav>
  );
}
