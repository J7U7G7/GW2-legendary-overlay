import { create } from "zustand";

import { api } from "../lib/tauri";
import type { AppearanceSettings } from "../types/gw2";

const DEFAULTS: AppearanceSettings = {
  opacity: 0.85,
  accent_color: "#7fb069",
  text_color: "#e8e8e8",
  background_color: "#000000",
  font_size: 12,
};

type SettingsStore = {
  appearance: AppearanceSettings;
  loaded: boolean;
  load: () => Promise<void>;
  update: (patch: Partial<AppearanceSettings>) => Promise<void>;
  reset: () => Promise<void>;
};

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const c = hex.replace("#", "");
  return {
    r: parseInt(c.slice(0, 2), 16) || 0,
    g: parseInt(c.slice(2, 4), 16) || 0,
    b: parseInt(c.slice(4, 6), 16) || 0,
  };
}

function applyToDom(a: AppearanceSettings) {
  const root = document.documentElement.style;
  root.setProperty("--accent-color", a.accent_color);
  root.setProperty("--text-color", a.text_color);
  root.setProperty("--bg-opacity", String(a.opacity));
  const { r, g, b } = hexToRgb(a.background_color);
  root.setProperty("--bg-color-rgba", `rgba(${r}, ${g}, ${b}, ${a.opacity})`);
  // Most of the UI uses explicit Tailwind pixel sizes (text-[10px],
  // etc.), so changing document.body.fontSize does nothing visible.
  // We expose `--ui-scale` and let each window's *content* div
  // (NOT the header — buttons there must stay clickable + visible)
  // apply zoom: var(--ui-scale). 12 is the baseline (matches text-xs).
  root.setProperty("--ui-scale", String(a.font_size / 12));
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  appearance: DEFAULTS,
  loaded: false,

  async load() {
    try {
      const a = await api.getAppearance();
      set({ appearance: a, loaded: true });
      applyToDom(a);
    } catch (e) {
      console.warn("getAppearance failed, using defaults:", e);
      set({ appearance: DEFAULTS, loaded: true });
      applyToDom(DEFAULTS);
    }
  },

  async update(patch) {
    const next = { ...get().appearance, ...patch };
    set({ appearance: next });
    applyToDom(next);
    try {
      await api.setAppearance(next);
    } catch (e) {
      console.warn("setAppearance failed:", e);
    }
  },

  async reset() {
    await get().update(DEFAULTS);
  },
}));
