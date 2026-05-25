import { useEffect } from "react";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";

import { api } from "../lib/tauri";
import type { HotkeyConfig } from "../types/gw2";

export const HOTKEY_DEFAULTS: HotkeyConfig = {
  toggle_visibility: "CmdOrCtrl+Shift+G",
  toggle_clickthrough: "CmdOrCtrl+Shift+H",
  toggle_bosses: "CmdOrCtrl+Shift+B",
  toggle_achievements: "CmdOrCtrl+Shift+P",
};

// Module-level state: we want one canonical "is the overlay click-through?"
// flag shared across the (potentially re-mounted) hook so a second mount
// doesn't desync the state. Tauri's setIgnoreCursorEvents takes a bool, but
// it has no getter, so we have to remember it ourselves.
let clickThroughOn = false;

async function toggleVisibility() {
  const w = getCurrentWindow();
  if (await w.isVisible()) {
    await w.hide();
  } else {
    await w.show();
    await w.setFocus();
  }
}

async function toggleClickThrough() {
  clickThroughOn = !clickThroughOn;
  await getCurrentWindow().setIgnoreCursorEvents(clickThroughOn);
}

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

async function bind(config: HotkeyConfig) {
  await unregisterAll();
  const safeRegister = async (accel: string, handler: () => void) => {
    const trimmed = accel.trim();
    if (!trimmed) return;
    try {
      await register(trimmed, (e) => {
        if (e.state === "Pressed") handler();
      });
    } catch (err) {
      console.warn(`hotkey '${trimmed}' failed to register:`, err);
    }
  };
  await safeRegister(config.toggle_visibility, () => void toggleVisibility());
  await safeRegister(config.toggle_clickthrough, () => void toggleClickThrough());
  await safeRegister(config.toggle_bosses, () => void toggleWindowByLabel("bosses"));
  await safeRegister(config.toggle_achievements, () =>
    void toggleWindowByLabel("achievements"),
  );
}

export function useHotkeys() {
  useEffect(() => {
    let cancelled = false;
    let unlistenHotkeysChanged: (() => void) | null = null;

    const setup = async () => {
      try {
        const cfg = await api.getHotkeys();
        if (cancelled) return;
        await bind(cfg);
        // Re-bind on hotkeys_changed broadcast from cmd_set_hotkeys.
        unlistenHotkeysChanged = await listen<HotkeyConfig>(
          "hotkeys_changed",
          (e) => {
            if (cancelled) return;
            void bind(e.payload).catch((err) =>
              console.warn("hotkey re-bind failed:", err),
            );
          },
        );
      } catch (err) {
        console.warn("hotkey setup failed:", err);
      }
    };
    void setup();

    return () => {
      cancelled = true;
      if (unlistenHotkeysChanged) unlistenHotkeysChanged();
      void unregisterAll().catch(() => {
        // ignore
      });
    };
  }, []);
}
