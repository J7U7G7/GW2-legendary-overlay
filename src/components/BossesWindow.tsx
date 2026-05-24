import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { useCollapse } from "../hooks/useCollapse";
import { useCrossWindowSync } from "../hooks/useCrossWindowSync";
import { useAppStore } from "../store/app";
import { useSettingsStore } from "../store/settings";
import { BossesView } from "./PinnedPanel";

export function BossesWindow() {
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);
  const checkApiKey = useAppStore((s) => s.checkApiKey);
  const loadSettings = useSettingsStore((s) => s.load);
  const { collapsed, toggle: toggleCollapse } = useCollapse();

  useCrossWindowSync();

  useEffect(() => {
    void checkApiKey();
    void loadSettings();
  }, [checkApiKey, loadSettings]);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.buttons === 1 && (e.target as HTMLElement).closest("[data-drag]")) {
      e.preventDefault();
      void getCurrentWindow().startDragging();
    }
  };

  return (
    <main
      className="h-screen w-screen flex flex-col text-[var(--text-color)] overflow-hidden"
      style={{ backgroundColor: "var(--bg-color-rgba, rgba(0, 0, 0, 0.85))" }}
    >
      <header
        className="flex items-center justify-between border-b border-white/10 shrink-0"
        onMouseDown={onMouseDown}
      >
        <div
          data-drag="1"
          data-tauri-drag-region
          className="flex-1 px-3 py-1.5 text-xs font-semibold cursor-grab active:cursor-grabbing"
        >
          🐉 Pinned Bosses
        </div>
        <div className="flex items-center gap-1 px-2">
          <button
            type="button"
            onClick={toggleCollapse}
            className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
            title={collapsed ? "Expand window" : "Collapse to header bar"}
          >
            {collapsed ? "▾" : "▴"}
          </button>
          <button
            type="button"
            onClick={() => void getCurrentWindow().hide()}
            className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
            title="Hide window (re-open via the 🐉 button on the main overlay)"
          >
            ✕
          </button>
        </div>
      </header>
      {!collapsed && (
        <div className="ui-zoom flex-1 flex flex-col overflow-hidden">
          {apiKeyStatus ? (
            <BossesView />
          ) : (
            <div className="flex-1 flex items-center justify-center text-xs opacity-60 px-4 text-center">
              Configure your API key in the main overlay first.
            </div>
          )}
        </div>
      )}
    </main>
  );
}
