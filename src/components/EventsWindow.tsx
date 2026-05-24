import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { useAppStore } from "../store/app";
import { EventsTab } from "./EventsTab";

export function EventsWindow() {
  const apiKeyStatus = useAppStore((s) => s.apiKeyStatus);
  const checkApiKey = useAppStore((s) => s.checkApiKey);
  const setView = useAppStore((s) => s.setView);

  useEffect(() => {
    void checkApiKey();
    setView("events"); // triggers initial events fetch
  }, [checkApiKey, setView]);

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
          GW2 Events
        </div>
        <button
          type="button"
          onClick={() => void getCurrentWindow().hide()}
          className="px-2 py-0.5 text-xs opacity-50 hover:opacity-100"
          title="Hide events window (show again from the main overlay)"
        >
          ✕
        </button>
      </header>
      {apiKeyStatus ? (
        <EventsTab />
      ) : (
        <div className="flex-1 flex items-center justify-center text-xs opacity-60 px-4 text-center">
          Configure your API key in the main overlay first.
        </div>
      )}
    </main>
  );
}
