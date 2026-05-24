// Mirrors of the Rust types returned by Tauri commands.

export type ApiKeyStatus = {
  account_id: string;
  permissions: string[];
  permissions_ok: boolean;
  missing: string[];
};

export type UpcomingEvent = {
  id: string;
  name: string;
  map: string;
  kind: "world_boss" | "meta_phase";
  start_at: string; // ISO 8601 UTC
  duration_minutes: number;
  waypoint_code: string | null;
};

export type WizardsVaultObjective = {
  id: number;
  title: string;
  track: string;
  acclaim: number;
  progress_current: number;
  progress_complete: number;
  claimed: boolean;
};

export type WizardsVaultPeriod = {
  period_type: "daily" | "weekly" | "special";
  period_start: string; // YYYY-MM-DD
  objectives: WizardsVaultObjective[];
};

export type WizardsVaultState = {
  daily: WizardsVaultPeriod | null;
  weekly: WizardsVaultPeriod | null;
  special: WizardsVaultPeriod | null;
};

export type ProgressSummary = {
  total_achievements_in_cache: number;
  account_tracked: number;
  account_done: number;
  points_earned: number;
};

export type SyncReport = {
  progress_changes: number;
  wv_daily: number;
  wv_weekly: number;
  wv_special: number;
};

export type AchievementSearchResult = {
  id: number;
  name: string;
  description: string | null;
  points: number;
  pinned: boolean;
};

export type LegendaryCollectionMember = {
  achievement_id: number;
  step: number;
  name: string;
  points: number;
  pinned: boolean;
  completion_ratio: number;
  done: boolean;
};

export type LegendaryCollection = {
  key: string;
  name: string;
  generation: string;
  kind: string;
  sort_order: number;
  members: LegendaryCollectionMember[];
  pinned_count: number;
  done_count: number;
};

export type PinnedItem = {
  id: number;
  name: string;
  description: string | null;
  current: number | null;
  max: number | null;
  completion_ratio: number;
  done: boolean;
  points: number;
  collection_key: string | null;
  associated_boss: string | null;
  next_event: UpcomingEvent | null;
  score: number;
};
