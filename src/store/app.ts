import { create } from "zustand";

import { api } from "../lib/tauri";
import type {
  ApiKeyStatus,
  ProgressSummary,
  UpcomingEvent,
  WizardsVaultState,
} from "../types/gw2";

type LoadingState = "idle" | "checking" | "syncing" | "error";

type AppStore = {
  apiKeyStatus: ApiKeyStatus | null;
  status: LoadingState;
  errorMessage: string | null;

  upcoming: UpcomingEvent[];
  wizardsVault: WizardsVaultState | null;
  summary: ProgressSummary | null;

  checkApiKey: () => Promise<void>;
  setApiKey: (key: string) => Promise<void>;
  clearApiKey: () => Promise<void>;
  refresh: () => Promise<void>;
  triggerSync: () => Promise<void>;
};

const HORIZON_MINUTES = 180;

export const useAppStore = create<AppStore>((set, get) => ({
  apiKeyStatus: null,
  status: "idle",
  errorMessage: null,
  upcoming: [],
  wizardsVault: null,
  summary: null,

  async checkApiKey() {
    set({ status: "checking", errorMessage: null });
    try {
      const status = await api.checkApiKey();
      set({ apiKeyStatus: status, status: "idle" });
      if (status && status.permissions_ok) {
        await get().refresh();
      }
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },

  async setApiKey(key: string) {
    set({ status: "checking", errorMessage: null });
    try {
      const status = await api.setApiKey(key);
      set({ apiKeyStatus: status, status: "idle" });
      // Backend already started a fresh engine; pull what's available so the UI
      // shows something, even if the first remote sync hasn't completed yet.
      await get().refresh();
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },

  async clearApiKey() {
    await api.clearApiKey();
    set({
      apiKeyStatus: null,
      upcoming: [],
      wizardsVault: null,
      summary: null,
      status: "idle",
      errorMessage: null,
    });
  },

  async refresh() {
    try {
      const [upcoming, wizardsVault, summary] = await Promise.all([
        api.getUpcomingEvents(HORIZON_MINUTES),
        api.getWizardsVaultState(),
        api.getProgressSummary(),
      ]);
      set({ upcoming, wizardsVault, summary, status: "idle", errorMessage: null });
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },

  async triggerSync() {
    set({ status: "syncing", errorMessage: null });
    try {
      await api.syncNow();
      await get().refresh();
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },
}));
