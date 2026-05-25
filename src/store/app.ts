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
export type ViewKey =
  | "events"
  | "catalog"
  | "search"
  | "wv"
  | "items"
  | "todos"
  | "builds";

type AppStore = {
  apiKeyStatus: ApiKeyStatus | null;
  /** True once `checkApiKey()` has run at least once (success or failure).
   * UI must gate the ApiKeySetup / no-key fallback on this — otherwise users
   * see the setup screen flash during the ~1s async check and re-type their
   * key, clobbering the perfectly-good stored value. */
  apiKeyChecked: boolean;
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
  removeBossGroup: (bossId: string) => Promise<void>;
};

export const useAppStore = create<AppStore>((set, get) => ({
  apiKeyStatus: null,
  apiKeyChecked: false,
  status: "idle",
  errorMessage: null,
  view: "events",

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
      set({ apiKeyStatus: status, status: "idle", apiKeyChecked: true });
      if (status && status.permissions_ok) {
        await get().refresh();
        try {
          const requested = await api.warmItemCache();
          if (requested > 0) await get().refresh();
        } catch (e) {
          console.warn("warmItemCache failed:", e);
        }
      }
    } catch (e) {
      // Crucially: do NOT clobber apiKeyStatus here. A transient backend
      // error (DPAPI/network/tokeninfo blip) must not kick the user to
      // the ApiKeySetup screen. Only an explicit `null` from
      // cmd_check_api_key means "no key configured". We DO mark the check
      // as completed so the Loading spinner can clear (else it spins
      // forever on a permanently broken check).
      console.error("checkApiKey failed:", e);
      set({ status: "error", errorMessage: String(e), apiKeyChecked: true });
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
      // Always re-fetch any view that was already loaded once. We don't
      // gate on the *current* view because pin/unpin actions can flow from
      // any tab and need to keep the others coherent.
      const hadCollections = get().collections.length > 0;
      const hadEvents = get().events.length > 0;
      const hasSearch = get().searchQuery.trim().length > 0;
      if (hadCollections) {
        const collections = await api.listLegendaryCollections();
        set({ collections });
      }
      if (hadEvents) {
        const events = await api.listEvents();
        set({ events });
      }
      if (hasSearch) {
        await get().runSearch();
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
    // Fetch any new item names referenced by this pin so the next
    // refresh resolves them; ignore failures (the UI falls back to ids).
    try {
      const fetched = await api.warmItemCache();
      if (fetched > 0) await get().refresh();
    } catch {
      // ignore
    }
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

  async removeBossGroup(bossId) {
    await api.removeBossGroup(bossId);
    await get().refresh();
  },
}));
