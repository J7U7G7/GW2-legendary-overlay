import { invoke } from "@tauri-apps/api/core";

import type {
  AchievementSearchResult,
  ApiKeyStatus,
  EventView,
  LegendaryCollection,
  PinnedView,
  ProgressSummary,
  SyncReport,
  UpcomingEvent,
  WizardsVaultState,
} from "../types/gw2";

export const api = {
  setApiKey: (key: string) => invoke<ApiKeyStatus>("cmd_set_api_key", { key }),
  checkApiKey: () => invoke<ApiKeyStatus | null>("cmd_check_api_key"),
  clearApiKey: () => invoke<void>("cmd_clear_api_key"),
  syncNow: () => invoke<SyncReport>("cmd_sync_now"),
  getUpcomingEvents: (horizonMinutes: number) =>
    invoke<UpcomingEvent[]>("cmd_get_upcoming_events", { horizonMinutes }),
  getWizardsVaultState: () => invoke<WizardsVaultState>("cmd_get_wizardsvault_state"),
  getProgressSummary: () => invoke<ProgressSummary>("cmd_get_progress_summary"),
  searchAchievements: (query: string, limit = 30) =>
    invoke<AchievementSearchResult[]>("cmd_search_achievements", { query, limit }),
  pinAchievement: (achievementId: number, collectionKey: string | null = null) =>
    invoke<void>("cmd_pin_achievement", { achievementId, collectionKey }),
  unpinAchievement: (achievementId: number) =>
    invoke<void>("cmd_unpin_achievement", { achievementId }),
  listLegendaryCollections: () =>
    invoke<LegendaryCollection[]>("cmd_list_legendary_collections"),
  getPinnedView: () => invoke<PinnedView>("cmd_get_pinned_view"),
  pinBoss: (bossId: string) => invoke<void>("cmd_pin_boss", { bossId }),
  unpinBoss: (bossId: string) => invoke<void>("cmd_unpin_boss", { bossId }),
  removeBossGroup: (bossId: string) => invoke<void>("cmd_remove_boss_group", { bossId }),
  listEvents: () => invoke<EventView[]>("cmd_list_events"),
};
