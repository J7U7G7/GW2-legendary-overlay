import { useEffect } from "react";
import {
  isRegistered,
  register,
  unregisterAll,
} from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWindow } from "@tauri-apps/api/window";

const TOGGLE_VISIBILITY = "CmdOrCtrl+Shift+G";
const TOGGLE_CLICKTHROUGH = "CmdOrCtrl+Shift+H";

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

export function useHotkeys() {
  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      try {
        // Clear any leftover bindings from a previous reload (Vite HMR).
        await unregisterAll();
        if (cancelled) return;
        await register(TOGGLE_VISIBILITY, (e) => {
          if (e.state === "Pressed") void toggleVisibility();
        });
        await register(TOGGLE_CLICKTHROUGH, (e) => {
          if (e.state === "Pressed") void toggleClickThrough();
        });
      } catch (err) {
        console.warn("hotkey registration failed:", err);
      }
    };
    void setup();
    return () => {
      cancelled = true;
      void unregisterAll().catch(() => {
        // ignore
      });
    };
  }, []);
}

export function getClickThroughState(): boolean {
  return clickThroughOn;
}

export async function isHotkeyRegistered(shortcut: string): Promise<boolean> {
  return isRegistered(shortcut);
}

export const HOTKEY_LABELS = {
  toggleVisibility: TOGGLE_VISIBILITY,
  toggleClickThrough: TOGGLE_CLICKTHROUGH,
};
