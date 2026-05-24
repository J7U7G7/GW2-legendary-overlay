import { invoke } from "@tauri-apps/api/core";

import type {
  ApiKeyStatus,
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
};
