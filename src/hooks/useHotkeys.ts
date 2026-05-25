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
  try {
    await unregisterAll();
  } catch (err) {
    console.warn("unregisterAll failed (continuing):", err);
  }
  // Try each shortcut, fall back to the hard-coded default if the user's
  // configured value rejects (bad combo, conflict, etc.). Without this
  // fallback a single mis-captured accelerator from the Settings panel kills
  // every other hotkey in the bind.
  const tryBind = async (configured: string, fallback: string, handler: () => void) => {
    const candidates = Array.from(
      new Set([configured.trim(), fallback].filter((s) => s.length > 0)),
    );
    for (const accel of candidates) {
      try {
        await register(accel, (e) => {
          if (e.state === "Pressed") handler();
        });
        return; // registered, stop trying alternates
      } catch (err) {
        console.warn(`hotkey '${accel}' failed to register:`, err);
      }
    }
    console.warn(
      `all candidates for this action failed (configured='${configured}', fallback='${fallback}')`,
    );
  };
  await tryBind(
    config.toggle_visibility,
    HOTKEY_DEFAULTS.toggle_visibility,
    () => void toggleVisibility(),
  );
  await tryBind(
    config.toggle_clickthrough,
    HOTKEY_DEFAULTS.toggle_clickthrough,
    () => void toggleClickThrough(),
  );
  await tryBind(
    config.toggle_bosses,
    HOTKEY_DEFAULTS.toggle_bosses,
    () => void toggleWindowByLabel("bosses"),
  );
  await tryBind(
    config.toggle_achievements,
    HOTKEY_DEFAULTS.toggle_achievements,
    () => void toggleWindowByLabel("achievements"),
  );
}

export function useHotkeys() {
  useEffect(() => {
    let cancelled = false;
    let unlistenHotkeysChanged: (() => void) | null = null;

    const setup = async () => {
      // Never let an error in getHotkeys leave the user without any global
      // shortcuts — fall back to the hard-coded defaults if the backend
      // command throws or returns junk. The bind() function additionally
      // falls back per-shortcut if a specific configured combo rejects.
      let cfg: HotkeyConfig = HOTKEY_DEFAULTS;
      try {
        cfg = await api.getHotkeys();
      } catch (err) {
        console.warn("getHotkeys failed, using built-in defaults:", err);
      }
      if (cancelled) return;
      try {
        await bind(cfg);
      } catch (err) {
        console.warn("hotkey bind failed entirely:", err);
      }
      try {
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
        console.warn("hotkeys_changed listener failed:", err);
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
