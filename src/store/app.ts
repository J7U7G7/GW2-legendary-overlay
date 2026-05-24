import { create } from "zustand";

import { api } from "../lib/tauri";
import type {
  AchievementSearchResult,
  ApiKeyStatus,
  EventView,
  LegendaryCollection,
  PinnedView,
  ProgressSummary,
  WizardsVaultState,
} from "../types/gw2";

type LoadingState = "idle" | "checking" | "syncing" | "error";
export type ViewKey = "pinned" | "events" | "catalog" | "search" | "wv";

type AppStore = {
  apiKeyStatus: ApiKeyStatus | null;
  status: LoadingState;
  errorMessage: string | null;
  view: ViewKey;

  wizardsVault: WizardsVaultState | null;
  summary: ProgressSummary | null;

  pinned: PinnedView | null;
  collections: LegendaryCollection[];
  events: EventView[];
  searchQuery: string;
  searchResults: AchievementSearchResult[];

  setView: (view: ViewKey) => void;
  checkApiKey: () => Promise<void>;
  setApiKey: (key: string) => Promise<void>;
  clearApiKey: () => Promise<void>;
  refresh: () => Promise<void>;
  triggerSync: () => Promise<void>;

  setSearchQuery: (q: string) => void;
  runSearch: () => Promise<void>;
  pin: (id: number, collectionKey?: string | null) => Promise<void>;
  unpin: (id: number) => Promise<void>;
  pinBoss: (bossId: string) => Promise<void>;
  unpinBoss: (bossId: string) => Promise<void>;
};

export const useAppStore = create<AppStore>((set, get) => ({
  apiKeyStatus: null,
  status: "idle",
  errorMessage: null,
  view: "pinned",

  wizardsVault: null,
  summary: null,

  pinned: null,
  collections: [],
  events: [],
  searchQuery: "",
  searchResults: [],

  setView(view) {
    set({ view });
    if (view === "catalog" && get().collections.length === 0) {
      void api.listLegendaryCollections().then((collections) => set({ collections }));
    }
    if (view === "events") {
      void api.listEvents().then((events) => set({ events }));
    }
  },

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
      await get().refresh();
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },

  async clearApiKey() {
    await api.clearApiKey();
    set({
      apiKeyStatus: null,
      wizardsVault: null,
      summary: null,
      pinned: null,
      collections: [],
      events: [],
      searchResults: [],
      searchQuery: "",
      status: "idle",
      errorMessage: null,
    });
  },

  async refresh() {
    try {
      const [wizardsVault, summary, pinned] = await Promise.all([
        api.getWizardsVaultState(),
        api.getProgressSummary(),
        api.getPinnedView(),
      ]);
      set({ wizardsVault, summary, pinned, status: "idle", errorMessage: null });
      const view = get().view;
      if (view === "catalog") {
        const collections = await api.listLegendaryCollections();
        set({ collections });
      }
      if (view === "events") {
        const events = await api.listEvents();
        set({ events });
      }
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

  setSearchQuery(q) {
    set({ searchQuery: q });
  },

  async runSearch() {
    const q = get().searchQuery.trim();
    if (q.length === 0) {
      set({ searchResults: [] });
      return;
    }
    try {
      const results = await api.searchAchievements(q, 30);
      set({ searchResults: results });
    } catch (e) {
      set({ status: "error", errorMessage: String(e) });
    }
  },

  async pin(id, collectionKey = null) {
    await api.pinAchievement(id, collectionKey);
    await get().refresh();
    if (get().searchQuery.length > 0) await get().runSearch();
  },

  async unpin(id) {
    await api.unpinAchievement(id);
    await get().refresh();
    if (get().searchQuery.length > 0) await get().runSearch();
  },

  async pinBoss(bossId) {
    await api.pinBoss(bossId);
    await get().refresh();
  },

  async unpinBoss(bossId) {
    await api.unpinBoss(bossId);
    await get().refresh();
  },
}));
