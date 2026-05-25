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

export type Build = {
  id: string;
  profession: string;
  elite_spec: string | null;
  role: string;
  name: string;
  source: string;
  source_url: string;
  chat_code: string;
  game_mode: string;
  gear_summary: string | null;
  weapons: string | null;
  difficulty: number | null;
  notes: string | null;
};

export type TodoView = {
  id: number;
  text: string;
  period: "daily" | "weekly";
  completed: boolean;
};

export type AccountItemLocation = {
  location: string;
  location_detail: string | null;
  count: number;
};

export type AccountItemResult = {
  item_id: number;
  name: string;
  rarity: string | null;
  total: number;
  locations: AccountItemLocation[];
};

export type AccountCurrencyResult = {
  currency_id: number;
  name: string;
  description: string | null;
  icon: string | null;
  value: number;
};

export type AppearanceSettings = {
  opacity: number;
  accent_color: string;
  text_color: string;
  background_color: string;
  font_size: number;
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

export type PinnedBit = {
  index: number;
  kind: string;
  ref_id: number | null;
  text: string | null;
  done: boolean;
  resolved_name: string | null;
  resolved_description: string | null;
  resolved_rarity: string | null;
};

export type PinnedItem = {
  id: number;
  name: string;
  description: string | null;
  requirement: string | null;
  current: number | null;
  max: number | null;
  completion_ratio: number;
  done: boolean;
  points: number;
  collection_key: string | null;
  associated_boss: string | null;
  next_event: UpcomingEvent | null;
  score: number;
  bits: PinnedBit[];
  is_pinned: boolean;
};

export type PinnedBossGroup = {
  boss_id: string;
  boss_name: string;
  boss_map: string;
  expansion: string;
  next_spawn: string; // ISO
  duration_minutes: number;
  waypoint_code: string | null;
  explicitly_pinned: boolean;
  achievements: PinnedItem[];
  has_remaining: boolean;
};

export type PinnedView = {
  boss_groups: PinnedBossGroup[];
  standalone: PinnedItem[];
};

export type EventKind = "world_boss" | "meta_event" | "ley_line";

export type EventView = {
  id: string;
  name: string;
  expansion: string;
  kind: EventKind;
  map: string;
  waypoint_code: string | null;
  next_spawn: string; // ISO
  duration_minutes: number;
  pinned: boolean;
};
