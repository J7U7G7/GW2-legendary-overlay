import { invoke } from "@tauri-apps/api/core";

import type {
  AccountCurrencyResult,
  AccountItemResult,
  AchievementSearchResult,
  ApiKeyStatus,
  AppearanceSettings,
  Build,
  EventView,
  LegendaryCollection,
  LegendaryProgress,
  PinnedView,
  ProgressSummary,
  SyncReport,
  TodoView,
  WizardsVaultState,
} from "../types/gw2";

export const api = {
  setApiKey: (key: string) => invoke<ApiKeyStatus>("cmd_set_api_key", { key }),
  checkApiKey: () => invoke<ApiKeyStatus | null>("cmd_check_api_key"),
  clearApiKey: () => invoke<void>("cmd_clear_api_key"),
  syncNow: () => invoke<SyncReport>("cmd_sync_now"),
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
  warmItemCache: () => invoke<number>("cmd_warm_item_cache"),
  getAppearance: () => invoke<AppearanceSettings>("cmd_get_appearance"),
  setAppearance: (appearance: AppearanceSettings) =>
    invoke<void>("cmd_set_appearance", { appearance }),
  saveStateAndQuit: () => invoke<void>("cmd_save_state_and_quit"),
  testNotification: () => invoke<void>("cmd_test_notification"),
  getNotificationLead: () => invoke<number>("cmd_get_notification_lead"),
  setNotificationLead: (minutes: number) =>
    invoke<void>("cmd_set_notification_lead", { minutes }),
  syncAccountItems: () => invoke<number>("cmd_sync_account_items"),
  searchAccountItems: (query: string, limit = 30) =>
    invoke<AccountItemResult[]>("cmd_search_account_items", { query, limit }),
  syncWallet: () => invoke<number>("cmd_sync_wallet"),
  searchCurrencies: (query: string, limit = 30) =>
    invoke<AccountCurrencyResult[]>("cmd_search_currencies", { query, limit }),
  listTodos: (period: "daily" | "weekly") =>
    invoke<TodoView[]>("cmd_list_todos", { period }),
  addTodo: (text: string, period: "daily" | "weekly") =>
    invoke<number>("cmd_add_todo", { text, period }),
  toggleTodo: (id: number) => invoke<void>("cmd_toggle_todo", { id }),
  deleteTodo: (id: number) => invoke<void>("cmd_delete_todo", { id }),
  listBuilds: (profession?: string) =>
    invoke<Build[]>("cmd_list_builds", { profession: profession ?? null }),
  legendaryProgress: () => invoke<LegendaryProgress[]>("cmd_legendary_progress"),
};
