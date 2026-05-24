import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useAppStore } from "../store/app";
import { useSettingsStore } from "../store/settings";

/**
 * Each Tauri window runs its own JS context with its own Zustand store and
 * its own DOM. Mutations in one window are invisible to the others until
 * the backend fires an event everyone can listen for. This hook wires both
 * windows into the broadcast loop so a pin in main becomes visible in the
 * bosses window, and an opacity tweak in settings reaches every overlay.
 */
export function useCrossWindowSync() {
  const refresh = useAppStore((s) => s.refresh);
  const loadSettings = useSettingsStore((s) => s.load);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    void listen("pinned_changed", () => {
      void refresh();
    }).then((u) => unsubs.push(u));
    void listen("appearance_changed", () => {
      void loadSettings();
    }).then((u) => unsubs.push(u));
    return () => {
      for (const u of unsubs) u();
    };
  }, [refresh, loadSettings]);
}
