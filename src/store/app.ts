import { create } from "zustand";

import { api } from "../lib/tauri";
import type {
  AchievementSearchResult,
  ApiKeyStatus,
  LegendaryCollection,
  PinnedItem,
  ProgressSummary,
  UpcomingEvent,
  WizardsVaultState,
} from "../types/gw2";

type LoadingState = "idle" | "checking" | "syncing" | "error";
export type ViewKey = "pinned" | "catalog" | "search" | "wv";

type AppStore = {
  apiKeyStatus: ApiKeyStatus | null;
  status: LoadingState;
  errorMessage: string | null;
  view: ViewKey;

  upcoming: UpcomingEvent[];
  wizardsVault: WizardsVaultState | null;
  summary: ProgressSummary | null;

  pinned: PinnedItem[];
  collections: LegendaryCollection[];
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
};

const HORIZON_MINUTES = 240;

export const useAppStore = create<AppStore>((set, get) => ({
  apiKeyStatus: null,
  status: "idle",
  errorMessage: null,
  view: "pinned",

  upcoming: [],
  wizardsVault: null,
  summary: null,

  pinned: [],
  collections: [],
  searchQuery: "",
  searchResults: [],

  setView(view) {
    set({ view });
    if (view === "catalog" && get().collections.length === 0) {
      void api.listLegendaryCollections().then((collections) => set({ collections }));
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
      upcoming: [],
      wizardsVault: null,
      summary: null,
      pinned: [],
      collections: [],
      searchResults: [],
      searchQuery: "",
      status: "idle",
      errorMessage: null,
    });
  },

  async refresh() {
    try {
      const [upcoming, wizardsVault, summary, pinned] = await Promise.all([
        api.getUpcomingEvents(HORIZON_MINUTES),
        api.getWizardsVaultState(),
        api.getProgressSummary(),
        api.getPinnedView(),
      ]);
      set({ upcoming, wizardsVault, summary, pinned, status: "idle", errorMessage: null });
      if (get().view === "catalog") {
        const collections = await api.listLegendaryCollections();
        set({ collections });
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
}));
