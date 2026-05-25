use std::sync::{Arc, Mutex};

use rusqlite::params;
use serde::Serialize;
use tauri::{Emitter, State};
use tracing::info;

/// Event name emitted after any pin/unpin/boss-remove so every window
/// re-fetches its view instead of relying on its own zustand state.
const PINNED_CHANGED_EVENT: &str = "pinned_changed";
/// Same idea for the appearance settings — each window has its own DOM,
/// so the originating window has to broadcast.
const APPEARANCE_CHANGED_EVENT: &str = "appearance_changed";

fn emit_pinned_changed(app: &tauri::AppHandle) {
    if let Err(e) = app.emit(PINNED_CHANGED_EVENT, ()) {
        tracing::warn!(error = %e, "failed to emit pinned_changed");
    }
}

fn emit_appearance_changed(app: &tauri::AppHandle) {
    if let Err(e) = app.emit(APPEARANCE_CHANGED_EVENT, ()) {
        tracing::warn!(error = %e, "failed to emit appearance_changed");
    }
}

use crate::api::auth::{ApiKey, clear_api_key, load_api_key, store_api_key};
use crate::api::client::ApiClient;
use crate::api::endpoints::{self, TokenInfo};
use crate::db::repository::Db;
use crate::error::{AppError, Result};
use crate::scorer::ranking::{Scoreable, Weights, score as score_item};
use crate::sync::engine::SyncEngine;
use crate::sync::{progress, wizardsvault};
use crate::timers::engine::{UpcomingEvent, all_upcoming};
use crate::timers::schedule::Schedule;

/// Lives in Tauri's `State<>`. `engine` is `None` until a valid key is set.
pub struct AppState {
    pub db: Arc<Db>,
    pub engine: Mutex<Option<SyncEngine>>,
    pub schedule: Arc<Schedule>,
    pub app_handle: tauri::AppHandle,
}

#[derive(Serialize)]
pub struct ApiKeyStatus {
    pub account_id: String,
    pub permissions: Vec<String>,
    pub permissions_ok: bool,
    pub missing: Vec<String>,
}

impl ApiKeyStatus {
    fn from(account_id: String, info: TokenInfo) -> Self {
        let missing = required_missing(&info);
        Self {
            account_id,
            permissions: info.permissions,
            permissions_ok: missing.is_empty(),
            missing,
        }
    }
}

fn required_missing(info: &TokenInfo) -> Vec<String> {
    let required = ["account", "progression", "unlocks", "inventories", "characters", "wallet"];
    required
        .into_iter()
        .filter(|p| !info.permissions.iter().any(|owned| owned == p))
        .map(String::from)
        .collect()
}

/// Validate the supplied key against `/v2/tokeninfo`, persist it (DPAPI), and
/// (re)start the sync engine on success.
#[tauri::command]
pub async fn cmd_set_api_key(state: State<'_, AppState>, key: String) -> Result<ApiKeyStatus> {
    let parsed = ApiKey::parse(&key)?;
    let probe = ApiClient::new(Some(parsed.clone()))?;
    let info = endpoints::get_tokeninfo(&probe).await?;
    let missing = required_missing(&info);
    if !missing.is_empty() {
        return Err(AppError::MissingPermissions(missing.join(", ")));
    }

    store_api_key(&state.db, &parsed)?;
    info!(account = parsed.account_id(), "API key stored");

    let mut engine_guard = state.engine.lock().expect("engine mutex poisoned");
    if let Some(prev) = engine_guard.take() {
        prev.shutdown();
    }
    let client = Arc::new(ApiClient::new(Some(parsed.clone()))?);
    let engine = SyncEngine::new(
        client,
        Arc::clone(&state.db),
        Arc::clone(&state.schedule),
        state.app_handle.clone(),
    );
    engine.start();
    *engine_guard = Some(engine);

    Ok(ApiKeyStatus::from(parsed.account_id().to_string(), info))
}

/// Returns the current key's status (None if no key configured). If the
/// key is in the DB but the network call to validate it against
/// `/v2/tokeninfo` fails, we still return an ApiKeyStatus marked
/// `permissions_ok = true` so the UI doesn't kick the user back to the
/// setup screen on a transient blip. The sync engine handles real auth
/// failures (401/403) by surfacing AppError::Unauthorized.
#[tauri::command]
pub async fn cmd_check_api_key(state: State<'_, AppState>) -> Result<Option<ApiKeyStatus>> {
    let Some(key) = load_api_key(&state.db)? else {
        tracing::debug!("check_api_key: no key stored");
        return Ok(None);
    };
    let account_id = key.account_id().to_string();
    let probe = ApiClient::new(Some(key.clone()))?;
    match endpoints::get_tokeninfo(&probe).await {
        Ok(info) => {
            tracing::debug!(account = %account_id, "check_api_key: validated");
            Ok(Some(ApiKeyStatus::from(account_id, info)))
        }
        Err(e) => {
            // Includes Unauthorized — ArenaNet's API throws transient 401s
            // during their frequent backend outages, and we don't want a
            // single bad response to kick the user back to the setup
            // screen. The periodic sync engine surfaces real auth failures
            // on its 5-min progress tick instead.
            tracing::warn!(
                error = %e,
                "check_api_key: validation failed, returning cached status",
            );
            Ok(Some(ApiKeyStatus {
                account_id,
                permissions: Vec::new(),
                permissions_ok: true,
                missing: Vec::new(),
            }))
        }
    }
}

#[tauri::command]
pub async fn cmd_clear_api_key(state: State<'_, AppState>) -> Result<()> {
    let mut engine_guard = state.engine.lock().expect("engine mutex poisoned");
    if let Some(prev) = engine_guard.take() {
        prev.shutdown();
    }
    clear_api_key(&state.db)?;
    info!("API key cleared");
    Ok(())
}

/// Trigger an out-of-band sync (progress + WV) without waiting for the next
/// interval tick. Achievement definitions bulk sync only runs at startup.
#[tauri::command]
pub async fn cmd_sync_now(state: State<'_, AppState>) -> Result<SyncReport> {
    // Grab the snapshot Arc and drop the guard before any await — a
    // std::sync::Mutex cannot be held across awaits.
    let snapshot = {
        let guard = state.engine.lock().expect("engine mutex poisoned");
        let Some(engine) = guard.as_ref() else {
            return Err(AppError::NoApiKey);
        };
        engine.snapshot()
    };

    let key = load_api_key(&state.db)?.ok_or(AppError::NoApiKey)?;
    let client = ApiClient::new(Some(key))?;

    let progress_changes = progress::sync_progress(&client, &state.db, &snapshot).await?;
    let wv_daily = wizardsvault::sync_daily(&client, &state.db).await?;
    let wv_weekly = wizardsvault::sync_weekly(&client, &state.db).await?;
    let wv_special = wizardsvault::sync_special(&client, &state.db).await?;

    Ok(SyncReport {
        progress_changes: progress_changes.len(),
        wv_daily,
        wv_weekly,
        wv_special,
    })
}

#[derive(Serialize)]
pub struct SyncReport {
    pub progress_changes: usize,
    pub wv_daily: usize,
    pub wv_weekly: usize,
    pub wv_special: usize,
}

// ============================================================================
// App lifecycle
// ============================================================================

/// Save every open window's position/size to disk and then exit the app.
/// Provides a graceful shutdown path that doesn't lose layout the way a
/// SIGINT from `tauri dev` would.
#[tauri::command]
pub async fn cmd_save_state_and_quit(app: tauri::AppHandle) -> Result<()> {
    use tauri_plugin_window_state::{AppHandleExt, StateFlags};
    if let Err(e) = app.save_window_state(StateFlags::all()) {
        tracing::warn!(error = %e, "save_window_state failed at quit");
    }
    app.exit(0);
    Ok(())
}

// ============================================================================
// Diagnostics — logs + version
// ============================================================================

fn logs_dir_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("com.tripleseptconsulting.gw2overlay").join("logs")
}

#[tauri::command]
pub async fn cmd_open_logs_folder(app: tauri::AppHandle) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = logs_dir_path();
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| AppError::Other(format!("open logs folder: {e}")))?;
    Ok(())
}

/// Return the most recent `max_lines` log lines from today's rolling file
/// (falls back to the latest yesterday file if today's hasn't been created
/// yet). Used by the 'Copy last errors' button in Settings — the user
/// pastes the result into a bug report.
#[tauri::command]
pub async fn cmd_recent_logs(max_lines: Option<usize>) -> Result<String> {
    let dir = logs_dir_path();
    let cap = max_lines.unwrap_or(100).min(2000);

    // Find the most recent file in the logs dir matching gw2-overlay.log.*
    let mut candidates: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| AppError::Other(format!("read logs dir: {e}")))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("gw2-overlay.log")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let mtime = meta.modified().ok()?;
            Some((mtime, e.path()))
        })
        .collect();
    candidates.sort_by_key(|(t, _)| *t);
    let Some((_, latest)) = candidates.into_iter().next_back() else {
        return Ok("(no log files yet)".to_string());
    };

    let content = std::fs::read_to_string(&latest)
        .map_err(|e| AppError::Other(format!("read log file: {e}")))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(cap);
    Ok(lines[start..].join("\n"))
}

#[tauri::command]
pub fn cmd_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Wipe the persisted window-state file so the next launch falls back to the
/// defaults declared in `tauri.conf.json` (centered, default sizes). Useful
/// when a window ends up off-screen / maximized / otherwise stuck — the
/// plugin has no programmatic 'reset' API of its own. Caller is expected to
/// relaunch the app afterwards via the process plugin.
#[tauri::command]
pub fn cmd_reset_window_layout() -> Result<()> {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let app_dir = base.join("com.tripleseptconsulting.gw2overlay");
    // tauri-plugin-window-state v2 writes to `.window-state` (no extension).
    // Be defensive against future renames + the legacy `.json` variant.
    for name in [".window-state", ".window-state.json", "window-state.json"] {
        let p = app_dir.join(name);
        if p.exists() {
            std::fs::remove_file(&p)
                .map_err(|e| AppError::Other(format!("remove {name}: {e}")))?;
        }
    }
    Ok(())
}

/// Wipe every data table (achievements, items, todos, wallet, etc.) while
/// preserving the API key + user preferences. Re-seeds the curated catalogs
/// (legendary_collections, boss links) so the Catalog tab isn't empty after
/// the wipe. The caller is expected to fire a sync afterwards to refill
/// from /v2/* — we emit `pinned_changed` so any open window's data view
/// re-fetches its now-empty state.
#[tauri::command]
pub async fn cmd_reset_database(state: State<'_, AppState>) -> Result<()> {
    state.db.wipe_data()?;
    // Re-seed the static catalogs (idempotent — uses INSERT OR REPLACE).
    crate::catalog::load_all(&state.db)?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

// ============================================================================
// Smart Legendary Selector (recipe walker, data/legendary_recipes.json)
// ============================================================================

#[tauri::command]
pub async fn cmd_legendary_progress(
    state: State<'_, AppState>,
) -> Result<Vec<crate::legendary::LegendaryProgress>> {
    use std::collections::HashMap;
    let catalog = crate::legendary::load()?;

    // Owned items: sum counts across every location (bank, materials, shared,
    // every character's bags + equipment).
    let owned_items: HashMap<u32, i64> = state.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT item_id, SUM(count) FROM account_items GROUP BY item_id",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)?))
        })?;
        let mut out: HashMap<u32, i64> = HashMap::new();
        for row in mapped {
            let (id, n) = row?;
            out.insert(id, n);
        }
        Ok(out)
    })?;

    let owned_currencies: HashMap<u32, i64> = state.db.with_conn(|c| {
        let mut stmt = c.prepare("SELECT currency_id, value FROM account_currencies")?;
        let mapped = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)?))
        })?;
        let mut out: HashMap<u32, i64> = HashMap::new();
        for row in mapped {
            let (id, v) = row?;
            out.insert(id, v);
        }
        Ok(out)
    })?;

    let mut progress: Vec<_> = catalog
        .legendaries
        .iter()
        .map(|rec| {
            crate::legendary::compute_progress(&catalog, rec, &owned_items, &owned_currencies, 5)
        })
        .collect();

    // Sort by completion ratio descending — "closest to done" first. Ties
    // broken by collection_key alphabetical.
    progress.sort_by(|a, b| {
        b.ratio
            .partial_cmp(&a.ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.collection_key.cmp(&b.collection_key))
    });

    Ok(progress)
}

#[tauri::command]
pub async fn cmd_list_builds(profession: Option<String>) -> Result<Vec<crate::builds::Build>> {
    let all = crate::builds::load_all()?;
    Ok(match profession {
        Some(p) => all
            .into_iter()
            .filter(|b| b.profession.eq_ignore_ascii_case(&p))
            .collect(),
        None => all,
    })
}

// ============================================================================
// Todos (daily / weekly with automatic reset)
// ============================================================================

#[derive(Serialize)]
pub struct TodoView {
    pub id: i64,
    pub text: String,
    pub period: String,
    pub completed: bool,
}

fn todo_period_start(period: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    use crate::sync::wizardsvault::{daily_period_start, weekly_period_start};
    let date = match period {
        "weekly" => weekly_period_start(now),
        _ => daily_period_start(now),
    };
    date.format("%Y-%m-%d").to_string()
}

fn reset_stale_todos(db: &Db, period: &str) -> Result<()> {
    let current = todo_period_start(period, chrono::Utc::now());
    db.with_conn(|c| {
        c.execute(
            "UPDATE todos
             SET completed_at = NULL, period_start = ?1
             WHERE period = ?2 AND period_start < ?1",
            params![current, period],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub async fn cmd_list_todos(state: State<'_, AppState>, period: String) -> Result<Vec<TodoView>> {
    reset_stale_todos(&state.db, &period)?;
    state.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT id, text, period, completed_at
             FROM todos
             WHERE period = ?1
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![period], |r| {
            let completed_at: Option<String> = r.get(3)?;
            Ok(TodoView {
                id: r.get(0)?,
                text: r.get(1)?,
                period: r.get(2)?,
                completed: completed_at.is_some(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}

#[tauri::command]
pub async fn cmd_add_todo(
    state: State<'_, AppState>,
    text: String,
    period: String,
) -> Result<i64> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::Other("todo text cannot be empty".into()));
    }
    if period != "daily" && period != "weekly" {
        return Err(AppError::Other("period must be 'daily' or 'weekly'".into()));
    }
    let start = todo_period_start(&period, chrono::Utc::now());
    state.db.with_conn(|c| {
        c.execute(
            "INSERT INTO todos (text, period, period_start) VALUES (?1, ?2, ?3)",
            params![text, &period, start],
        )?;
        Ok(c.last_insert_rowid())
    })
}

#[tauri::command]
pub async fn cmd_toggle_todo(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.with_conn(|c| {
        // Flip completed_at: NULL → now, set → NULL.
        c.execute(
            "UPDATE todos
             SET completed_at = CASE
                WHEN completed_at IS NULL THEN CURRENT_TIMESTAMP
                ELSE NULL
             END
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub async fn cmd_delete_todo(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.with_conn(|c| {
        c.execute("DELETE FROM todos WHERE id = ?1", params![id])?;
        Ok(())
    })
}

// ============================================================================
// Account inventory search (bank / materials / characters / shared)
// ============================================================================

#[derive(Serialize)]
pub struct AccountItemLocation {
    pub location: String,
    pub location_detail: Option<String>,
    pub count: u32,
}

#[derive(Serialize)]
pub struct AccountItemResult {
    pub item_id: u32,
    pub name: String,
    pub rarity: Option<String>,
    pub total: u32,
    pub locations: Vec<AccountItemLocation>,
}

#[tauri::command]
pub async fn cmd_sync_account_items(state: State<'_, AppState>) -> Result<usize> {
    let key = load_api_key(&state.db)?.ok_or(AppError::NoApiKey)?;
    let client = ApiClient::new(Some(key))?;
    let n = crate::sync::inventory::sync_account_items(&client, &state.db).await?;

    // Warm items_cache for every id we just inserted. We re-fetch in batches
    // even for ids already present so names migrate from any prior English
    // sync to the current French one. /v2/items batch is fast (200 ids per
    // call) and only runs on user-initiated sync.
    let ids = crate::sync::inventory::distinct_item_ids(&state.db)?;
    if !ids.is_empty() {
        let _ = crate::sync::items::fetch_and_cache_items(&client, &state.db, &ids).await;
    }
    Ok(n)
}

#[derive(Serialize)]
pub struct AccountCurrencyResult {
    pub currency_id: u32,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub value: i64,
}

#[tauri::command]
pub async fn cmd_sync_wallet(state: State<'_, AppState>) -> Result<usize> {
    let key = load_api_key(&state.db)?.ok_or(AppError::NoApiKey)?;
    let client = ApiClient::new(Some(key))?;
    crate::sync::wallet::sync_wallet(&client, &state.db).await
}

#[tauri::command]
pub async fn cmd_search_currencies(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<AccountCurrencyResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let like = format!("%{q}%");
    let limit = limit.unwrap_or(30).min(100) as i64;
    state.db.with_conn(|c| {
        // LEFT JOIN so a currency we have a definition for but no wallet entry
        // (extremely rare — would mean the currency was wiped from the wallet
        // mid-session) still shows up with value 0.
        let mut stmt = c.prepare(
            "SELECT c.id, c.name, c.description, c.icon, COALESCE(ac.value, 0)
             FROM currencies c
             LEFT JOIN account_currencies ac ON ac.currency_id = c.id
             WHERE c.name LIKE ?1 COLLATE NOCASE
             ORDER BY length(c.name), c.sort_order, c.name
             LIMIT ?2",
        )?;
        let rows: Vec<AccountCurrencyResult> = stmt
            .query_map(rusqlite::params![like, limit], |r| {
                Ok(AccountCurrencyResult {
                    currency_id: r.get::<_, i64>(0)? as u32,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    icon: r.get(3)?,
                    value: r.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    })
}

#[tauri::command]
pub async fn cmd_search_account_items(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<AccountItemResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let like = format!("%{q}%");
    let limit = limit.unwrap_or(30).min(100) as i64;
    state.db.with_conn(|c| {
        // Pull the matching item_ids ordered by name length (shorter ≈ closer
        // match) then by name. Then for each id, pull every location row.
        let mut id_stmt = c.prepare(
            "SELECT DISTINCT ai.item_id, ic.name, ic.rarity
             FROM account_items ai
             JOIN items_cache ic ON ic.id = ai.item_id
             WHERE ic.name LIKE ?1 COLLATE NOCASE
             ORDER BY length(ic.name), ic.name
             LIMIT ?2",
        )?;
        let mapped = id_stmt
            .query_map(rusqlite::params![like, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)? as u32,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        let mut loc_stmt = c.prepare(
            "SELECT location, location_detail, count
             FROM account_items
             WHERE item_id = ?1
             ORDER BY location, location_detail",
        )?;

        let mut out = Vec::with_capacity(mapped.len());
        for (id, name, rarity) in mapped {
            let locations: Vec<AccountItemLocation> = loc_stmt
                .query_map(rusqlite::params![id], |r| {
                    Ok(AccountItemLocation {
                        location: r.get(0)?,
                        location_detail: r.get(1)?,
                        count: r.get::<_, i64>(2)? as u32,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            let total = locations.iter().map(|l| l.count).sum();
            out.push(AccountItemResult {
                item_id: id,
                name,
                rarity,
                total,
                locations,
            });
        }
        Ok(out)
    })
}

/// Fire a sample notification so the user can verify the toast pipeline
/// works without waiting for a real boss spawn.
#[tauri::command]
pub async fn cmd_test_notification(app: tauri::AppHandle) -> Result<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title("GW2 Overlay")
        .body("Test notification — if you see this, toasts are wired up correctly.")
        .show()
        .map_err(|e| AppError::Other(format!("notification failed: {e}")))?;
    Ok(())
}

const NOTIFY_LEAD_KEY: &str = "notification_lead_minutes";
pub const DEFAULT_NOTIFY_LEAD_MINUTES: i64 = 2;

#[tauri::command]
pub async fn cmd_get_notification_lead(state: State<'_, AppState>) -> Result<i64> {
    Ok(state
        .db
        .get_setting(NOTIFY_LEAD_KEY)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_NOTIFY_LEAD_MINUTES))
}

#[tauri::command]
pub async fn cmd_set_notification_lead(state: State<'_, AppState>, minutes: i64) -> Result<()> {
    let clamped = minutes.clamp(1, 15);
    state.db.set_setting(NOTIFY_LEAD_KEY, &clamped.to_string())
}

// ============================================================================
// Hotkey configuration (persistent, user-remappable)
// ============================================================================

/// Default shortcut for each action. `accelerator` format from
/// `@tauri-apps/plugin-global-shortcut` — `CmdOrCtrl` resolves to Ctrl on
/// Windows / Cmd on macOS.
pub const DEFAULT_HOTKEY_TOGGLE_VISIBILITY: &str = "CmdOrCtrl+Shift+G";
pub const DEFAULT_HOTKEY_TOGGLE_CLICKTHROUGH: &str = "CmdOrCtrl+Shift+H";
pub const DEFAULT_HOTKEY_TOGGLE_BOSSES: &str = "CmdOrCtrl+Shift+B";
pub const DEFAULT_HOTKEY_TOGGLE_ACHIEVEMENTS: &str = "CmdOrCtrl+Shift+P";

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HotkeyConfig {
    pub toggle_visibility: String,
    pub toggle_clickthrough: String,
    pub toggle_bosses: String,
    pub toggle_achievements: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle_visibility: DEFAULT_HOTKEY_TOGGLE_VISIBILITY.into(),
            toggle_clickthrough: DEFAULT_HOTKEY_TOGGLE_CLICKTHROUGH.into(),
            toggle_bosses: DEFAULT_HOTKEY_TOGGLE_BOSSES.into(),
            toggle_achievements: DEFAULT_HOTKEY_TOGGLE_ACHIEVEMENTS.into(),
        }
    }
}

#[tauri::command]
pub async fn cmd_get_hotkeys(state: State<'_, AppState>) -> Result<HotkeyConfig> {
    let pick = |key: &str, fallback: &str| -> Result<String> {
        Ok(state
            .db
            .get_setting(key)?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| fallback.to_string()))
    };
    Ok(HotkeyConfig {
        toggle_visibility: pick("hotkey_toggle_visibility", DEFAULT_HOTKEY_TOGGLE_VISIBILITY)?,
        toggle_clickthrough: pick(
            "hotkey_toggle_clickthrough",
            DEFAULT_HOTKEY_TOGGLE_CLICKTHROUGH,
        )?,
        toggle_bosses: pick("hotkey_toggle_bosses", DEFAULT_HOTKEY_TOGGLE_BOSSES)?,
        toggle_achievements: pick(
            "hotkey_toggle_achievements",
            DEFAULT_HOTKEY_TOGGLE_ACHIEVEMENTS,
        )?,
    })
}

#[tauri::command]
pub async fn cmd_set_hotkeys(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    hotkeys: HotkeyConfig,
) -> Result<()> {
    state
        .db
        .set_setting("hotkey_toggle_visibility", &hotkeys.toggle_visibility)?;
    state
        .db
        .set_setting("hotkey_toggle_clickthrough", &hotkeys.toggle_clickthrough)?;
    state
        .db
        .set_setting("hotkey_toggle_bosses", &hotkeys.toggle_bosses)?;
    state
        .db
        .set_setting("hotkey_toggle_achievements", &hotkeys.toggle_achievements)?;
    // Broadcast so the main window can unregister + re-register the new
    // accelerators without a relaunch.
    let _ = app.emit("hotkeys_changed", &hotkeys);
    Ok(())
}

// ============================================================================
// Appearance settings (spec §5.6)
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AppearanceSettings {
    pub opacity: f64,
    pub accent_color: String,
    pub text_color: String,
    pub background_color: String,
    pub font_size: u32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            opacity: 0.85,
            accent_color: "#7fb069".into(),
            text_color: "#e8e8e8".into(),
            background_color: "#000000".into(),
            font_size: 12,
        }
    }
}

const APPEARANCE_KEY: &str = "appearance";

#[tauri::command]
pub async fn cmd_get_appearance(state: State<'_, AppState>) -> Result<AppearanceSettings> {
    let raw = state.db.get_setting(APPEARANCE_KEY)?;
    Ok(raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn cmd_set_appearance(
    state: State<'_, AppState>,
    appearance: AppearanceSettings,
) -> Result<()> {
    let json = serde_json::to_string(&appearance)?;
    state.db.set_setting(APPEARANCE_KEY, &json)?;
    emit_appearance_changed(&state.app_handle);
    Ok(())
}

// ============================================================================
// Read-only views for the UI
// ============================================================================

#[derive(Serialize)]
pub struct WizardsVaultObjectiveView {
    pub id: u32,
    pub title: String,
    pub track: String,
    pub acclaim: u32,
    pub progress_current: u32,
    pub progress_complete: u32,
    pub claimed: bool,
}

#[derive(Serialize)]
pub struct WizardsVaultPeriodView {
    pub period_type: String,
    pub period_start: String,
    pub objectives: Vec<WizardsVaultObjectiveView>,
}

#[derive(Serialize, Default)]
pub struct WizardsVaultStateView {
    pub daily: Option<WizardsVaultPeriodView>,
    pub weekly: Option<WizardsVaultPeriodView>,
    pub special: Option<WizardsVaultPeriodView>,
}

#[derive(Serialize)]
pub struct ProgressSummary {
    pub total_achievements_in_cache: i64,
    pub account_tracked: i64,
    pub account_done: i64,
    pub points_earned: i64,
}

#[tauri::command]
pub async fn cmd_get_wizardsvault_state(
    state: State<'_, AppState>,
) -> Result<WizardsVaultStateView> {
    let mut view = WizardsVaultStateView::default();
    for kind in [
        wizardsvault::PERIOD_DAILY,
        wizardsvault::PERIOD_WEEKLY,
        wizardsvault::PERIOD_SPECIAL,
    ] {
        let period = read_latest_period(&state.db, kind)?;
        match kind {
            wizardsvault::PERIOD_DAILY => view.daily = period,
            wizardsvault::PERIOD_WEEKLY => view.weekly = period,
            wizardsvault::PERIOD_SPECIAL => view.special = period,
            _ => {}
        }
    }
    Ok(view)
}

#[tauri::command]
pub async fn cmd_get_progress_summary(state: State<'_, AppState>) -> Result<ProgressSummary> {
    state.db.with_conn(|c| {
        let total_achievements_in_cache: i64 =
            c.query_row("SELECT COUNT(*) FROM achievements", [], |r| r.get(0))?;
        let account_tracked: i64 =
            c.query_row("SELECT COUNT(*) FROM account_progress", [], |r| r.get(0))?;
        let account_done: i64 =
            c.query_row("SELECT COUNT(*) FROM account_progress WHERE done = 1", [], |r| {
                r.get(0)
            })?;
        let points_earned: i64 = c
            .query_row(
                "SELECT COALESCE(SUM(a.points), 0)
                 FROM account_progress p
                 JOIN achievements a ON a.id = p.achievement_id
                 WHERE p.done = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(ProgressSummary {
            total_achievements_in_cache,
            account_tracked,
            account_done,
            points_earned,
        })
    })
}

// ============================================================================
// Pinning + search + ranked view
// ============================================================================

#[derive(Serialize)]
pub struct AchievementSearchResult {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub points: i64,
    pub pinned: bool,
}

#[derive(Serialize)]
pub struct LegendaryCollectionMemberView {
    pub achievement_id: u32,
    pub step: i64,
    pub name: String,
    pub points: i64,
    pub pinned: bool,
    pub completion_ratio: f64,
    pub done: bool,
}

#[derive(Serialize)]
pub struct LegendaryCollectionView {
    pub key: String,
    pub name: String,
    pub generation: String,
    pub kind: String,
    pub sort_order: i64,
    pub members: Vec<LegendaryCollectionMemberView>,
    pub pinned_count: usize,
    pub done_count: usize,
}

#[derive(Serialize, Clone)]
pub struct PinnedItemView {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub requirement: Option<String>,
    pub current: Option<i64>,
    pub max: Option<i64>,
    pub completion_ratio: f64,
    pub done: bool,
    pub points: i64,
    pub collection_key: Option<String>,
    pub associated_boss: Option<String>,
    pub next_event: Option<UpcomingEvent>,
    pub score: f64,
    pub bits: Vec<PinnedBitView>,
    /// True for items the user explicitly pinned. False for items that show
    /// up in a boss group only because the user pinned the boss and this
    /// achievement is linked to it.
    pub is_pinned: bool,
}

#[derive(Serialize, Clone)]
pub struct PinnedBitView {
    pub index: u32,
    pub kind: String,
    pub ref_id: Option<i64>,
    pub text: Option<String>,
    pub done: bool,
    /// Resolved name for Item-typed bits (looked up in items_cache).
    pub resolved_name: Option<String>,
    pub resolved_description: Option<String>,
    pub resolved_rarity: Option<String>,
}

#[derive(Serialize)]
pub struct PinnedBossGroup {
    pub boss_id: String,
    pub boss_name: String,
    pub boss_map: String,
    pub expansion: String,
    pub next_spawn: chrono::DateTime<chrono::Utc>,
    pub duration_minutes: u32,
    pub waypoint_code: Option<String>,
    pub explicitly_pinned: bool,
    pub achievements: Vec<PinnedItemView>,
    pub has_remaining: bool,
}

#[derive(Serialize)]
pub struct PinnedView {
    pub boss_groups: Vec<PinnedBossGroup>,
    pub standalone: Vec<PinnedItemView>,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    WorldBoss,
    MetaEvent,
    LeyLine,
}

#[derive(Serialize)]
pub struct EventView {
    pub id: String,
    pub name: String,
    pub expansion: String,
    pub kind: EventKind,
    pub map: String,
    pub waypoint_code: Option<String>,
    pub next_spawn: chrono::DateTime<chrono::Utc>,
    pub duration_minutes: u32,
    pub pinned: bool,
}

#[tauri::command]
pub async fn cmd_search_achievements(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<AchievementSearchResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let like = format!("%{q}%");
    let limit = limit.unwrap_or(30).min(100) as i64;
    state.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT a.id, a.name, a.description, COALESCE(a.points, 0),
                    CASE WHEN p.achievement_id IS NULL THEN 0 ELSE 1 END
             FROM achievements a
             LEFT JOIN pinned_achievements p ON p.achievement_id = a.id
             WHERE a.name LIKE ?1 COLLATE NOCASE
             ORDER BY length(a.name), a.name
             LIMIT ?2",
        )?;
        let mapped = stmt.query_map(params![like, limit], |r| {
            Ok(AchievementSearchResult {
                id: r.get::<_, i64>(0)? as u32,
                name: r.get(1)?,
                description: r.get(2)?,
                points: r.get(3)?,
                pinned: r.get::<_, i64>(4)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })
}

#[tauri::command]
pub async fn cmd_pin_achievement(
    state: State<'_, AppState>,
    achievement_id: u32,
    collection_key: Option<String>,
) -> Result<()> {
    state.db.pin_achievement(achievement_id, collection_key.as_deref())?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

#[tauri::command]
pub async fn cmd_unpin_achievement(
    state: State<'_, AppState>,
    achievement_id: u32,
) -> Result<()> {
    state.db.unpin_achievement(achievement_id)?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

#[tauri::command]
pub async fn cmd_list_legendary_collections(
    state: State<'_, AppState>,
) -> Result<Vec<LegendaryCollectionView>> {
    state.db.with_conn(|c| {
        let mut stmt_cols = c.prepare(
            "SELECT key, name, generation, kind, sort_order
             FROM legendary_collections
             ORDER BY sort_order, name",
        )?;
        let collections: Vec<(String, String, String, String, i64)> = stmt_cols
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut out = Vec::with_capacity(collections.len());
        for (key, name, gen, kind, order) in collections {
            let mut stmt_mem = c.prepare(
                "SELECT m.achievement_id, m.step, a.name, COALESCE(a.points, 0),
                        CASE WHEN pin.achievement_id IS NULL THEN 0 ELSE 1 END,
                        p.current, p.max, COALESCE(p.done, 0)
                 FROM legendary_collection_members m
                 LEFT JOIN achievements a ON a.id = m.achievement_id
                 LEFT JOIN pinned_achievements pin ON pin.achievement_id = m.achievement_id
                 LEFT JOIN account_progress p ON p.achievement_id = m.achievement_id
                 WHERE m.collection_key = ?1
                 ORDER BY m.step, m.achievement_id",
            )?;
            let mut members = Vec::new();
            let mut pinned_count = 0usize;
            let mut done_count = 0usize;
            let mapped = stmt_mem.query_map(params![key], |r| {
                let id: i64 = r.get(0)?;
                let step: i64 = r.get(1)?;
                let name: Option<String> = r.get(2)?;
                let points: i64 = r.get(3)?;
                let pinned: i64 = r.get(4)?;
                let current: Option<i64> = r.get(5)?;
                let max: Option<i64> = r.get(6)?;
                let done: i64 = r.get(7)?;
                Ok((id, step, name, points, pinned, current, max, done))
            })?;
            for row in mapped {
                let (id, step, name, points, pinned, current, max, done) = row?;
                if pinned != 0 {
                    pinned_count += 1;
                }
                if done != 0 {
                    done_count += 1;
                }
                let ratio = match (current, max) {
                    (Some(c), Some(m)) if m > 0 => (c as f64 / m as f64).clamp(0.0, 1.0),
                    _ => {
                        if done != 0 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                members.push(LegendaryCollectionMemberView {
                    achievement_id: id as u32,
                    step,
                    name: name.unwrap_or_else(|| format!("Achievement #{id}")),
                    points,
                    pinned: pinned != 0,
                    completion_ratio: ratio,
                    done: done != 0,
                });
            }
            out.push(LegendaryCollectionView {
                key,
                name,
                generation: gen,
                kind,
                sort_order: order,
                members,
                pinned_count,
                done_count,
            });
        }
        Ok(out)
    })
}

#[tauri::command]
pub async fn cmd_pin_boss(state: State<'_, AppState>, boss_id: String) -> Result<()> {
    state.db.pin_boss(&boss_id)?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

#[tauri::command]
pub async fn cmd_unpin_boss(state: State<'_, AppState>, boss_id: String) -> Result<()> {
    state.db.unpin_boss(&boss_id)?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

/// Wipe a boss group entirely: the explicit boss pin (if any) plus every
/// pinned achievement linked to that boss via achievement_metadata. Called
/// when the user dismisses a boss card from the Pinned tab.
#[tauri::command]
pub async fn cmd_remove_boss_group(state: State<'_, AppState>, boss_id: String) -> Result<()> {
    state.db.remove_boss_group(&boss_id)?;
    emit_pinned_changed(&state.app_handle);
    Ok(())
}

#[tauri::command]
pub async fn cmd_list_events(state: State<'_, AppState>) -> Result<Vec<EventView>> {
    use crate::timers::engine::{current_meta_phase, next_spawn, prev_spawn};
    let now = chrono::Utc::now();
    let pinned_set: std::collections::HashSet<String> =
        state.db.list_pinned_boss_ids()?.into_iter().collect();

    let mut out = Vec::new();
    for boss in &state.schedule.world_bosses {
        // If the boss spawned within its duration window, surface THAT
        // start time so the UI can render 'active Xm left' instead of
        // jumping to the next cycle.
        let start_at = prev_spawn(boss, now)
            .and_then(|prev| {
                let end = prev + chrono::Duration::minutes(boss.duration_minutes as i64);
                if now < end { Some(prev) } else { None }
            })
            .unwrap_or_else(|| next_spawn(boss, now));
        out.push(EventView {
            id: boss.id.clone(),
            name: boss.name.clone(),
            expansion: boss.expansion.clone(),
            kind: EventKind::WorldBoss,
            map: boss.map.clone(),
            waypoint_code: boss.waypoint_code.clone(),
            next_spawn: start_at,
            duration_minutes: boss.duration_minutes,
            pinned: pinned_set.contains(&boss.id),
        });
    }
    for meta in &state.schedule.meta_events {
        let instant = current_meta_phase(meta, now);
        // Active phase first, fallback to next.
        let (start_at, duration) = if let Some(active) = &instant.active {
            let dur = meta
                .phases
                .iter()
                .find(|p| p.name == active.name)
                .map(|p| p.duration_minutes)
                .unwrap_or(0);
            (active.started_at, dur)
        } else {
            let dur = meta
                .phases
                .iter()
                .find(|p| p.name == instant.next.name)
                .map(|p| p.duration_minutes)
                .unwrap_or(0);
            (instant.next.starts_at, dur)
        };
        out.push(EventView {
            id: meta.id.clone(),
            name: meta.name.clone(),
            expansion: meta.expansion.clone().unwrap_or_else(|| "Core".to_string()),
            kind: EventKind::MetaEvent,
            map: meta.map.clone(),
            waypoint_code: meta.waypoint_code.clone(),
            next_spawn: start_at,
            duration_minutes: duration,
            pinned: pinned_set.contains(&meta.id),
        });
    }
    if let Some(lla) = &state.schedule.ley_line_anomaly {
        // Ley-Line's "next_spawn" is the same calculation as a world boss with
        // fixed schedule_utc times.
        let lla_boss = crate::timers::schedule::WorldBoss {
            id: lla.id.clone(),
            name: lla.name.clone(),
            tier: None,
            map: lla.rotation_maps.first().cloned().unwrap_or_default(),
            area: None,
            waypoint_code: None,
            expansion: "Core".to_string(),
            schedule_utc: lla.schedule_utc.clone(),
            duration_minutes: lla.duration_minutes,
            wiki_event: None,
        };
        out.push(EventView {
            id: lla.id.clone(),
            name: lla.name.clone(),
            expansion: "Special".to_string(),
            kind: EventKind::LeyLine,
            map: lla.rotation_maps.join(" / "),
            waypoint_code: None,
            next_spawn: next_spawn(&lla_boss, now),
            duration_minutes: lla.duration_minutes,
            pinned: pinned_set.contains(&lla.id),
        });
    }

    // Sort: by expansion (Core first, then alpha), then by next_spawn ascending.
    out.sort_by(|a, b| {
        let exp_order = |e: &str| match e {
            "Core" => 0,
            "HoT" => 1,
            "PoF" => 2,
            "LWS3" => 3,
            "LWS4" => 4,
            "EoD" => 5,
            "SotO" => 6,
            "JW" => 7,
            "Special" => 99,
            _ => 50,
        };
        exp_order(&a.expansion)
            .cmp(&exp_order(&b.expansion))
            .then_with(|| a.next_spawn.cmp(&b.next_spawn))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

#[tauri::command]
pub async fn cmd_get_pinned_view(state: State<'_, AppState>) -> Result<PinnedView> {
    let now = chrono::Utc::now();
    let upcoming_window = all_upcoming(&state.schedule, now, 240);
    let weights = Weights::default();

    let pinned_boss_ids = state.db.list_pinned_boss_ids()?;
    let achievements = load_pinned_achievement_views(&state.db, &upcoming_window, &weights, now)?;

    // Group achievements by associated_boss
    use std::collections::HashMap;
    let mut by_boss: HashMap<String, Vec<PinnedItemView>> = HashMap::new();
    let mut standalone: Vec<PinnedItemView> = Vec::new();
    for item in achievements {
        match item.associated_boss.clone() {
            Some(b) => by_boss.entry(b).or_default().push(item),
            None => standalone.push(item),
        }
    }

    // For each explicitly-pinned boss, also load every other achievement
    // linked to that boss via achievement_metadata so the user sees the
    // complete done / to-do picture, not just what they pinned themselves.
    let already_in_groups: std::collections::HashSet<u32> = by_boss
        .values()
        .flat_map(|v| v.iter().map(|i| i.id))
        .collect();
    for boss_id in &pinned_boss_ids {
        let related = load_boss_linked_achievements(
            &state.db,
            boss_id,
            &upcoming_window,
            &weights,
            now,
            &already_in_groups,
        )?;
        if !related.is_empty() {
            by_boss.entry(boss_id.clone()).or_default().extend(related);
        }
    }

    // Union: event IDs from explicit pins + IDs referenced by pinned achievements
    let mut all_event_ids: Vec<String> = pinned_boss_ids.to_vec();
    for k in by_boss.keys() {
        if !all_event_ids.contains(k) {
            all_event_ids.push(k.clone());
        }
    }

    let mut boss_groups: Vec<PinnedBossGroup> = all_event_ids
        .into_iter()
        .filter_map(|event_id| {
            let info = lookup_event(&state.schedule, &event_id, now)?;
            let achievements = by_boss.remove(&event_id).unwrap_or_default();
            let has_remaining = achievements.iter().any(|a| !a.done);
            Some(PinnedBossGroup {
                boss_id: info.id,
                boss_name: info.name,
                boss_map: info.map,
                expansion: info.expansion,
                next_spawn: info.next_spawn,
                duration_minutes: info.duration_minutes,
                waypoint_code: info.waypoint_code,
                explicitly_pinned: pinned_boss_ids.contains(&event_id),
                achievements,
                has_remaining,
            })
        })
        .collect();

    boss_groups.sort_by_key(|g| g.next_spawn);
    standalone.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(PinnedView { boss_groups, standalone })
}

struct EventInfo {
    id: String,
    name: String,
    map: String,
    expansion: String,
    next_spawn: chrono::DateTime<chrono::Utc>,
    duration_minutes: u32,
    waypoint_code: Option<String>,
}

fn lookup_event(
    schedule: &Schedule,
    id: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<EventInfo> {
    use crate::timers::engine::{current_meta_phase, next_spawn, prev_spawn};
    if let Some(b) = schedule.world_bosses.iter().find(|b| b.id == id) {
        let start_at = prev_spawn(b, now)
            .and_then(|prev| {
                let end = prev + chrono::Duration::minutes(b.duration_minutes as i64);
                if now < end { Some(prev) } else { None }
            })
            .unwrap_or_else(|| next_spawn(b, now));
        return Some(EventInfo {
            id: b.id.clone(),
            name: b.name.clone(),
            map: b.map.clone(),
            expansion: b.expansion.clone(),
            next_spawn: start_at,
            duration_minutes: b.duration_minutes,
            waypoint_code: b.waypoint_code.clone(),
        });
    }
    if let Some(m) = schedule.meta_events.iter().find(|m| m.id == id) {
        let instant = current_meta_phase(m, now);
        let (start_at, duration) = if let Some(active) = &instant.active {
            let dur = m
                .phases
                .iter()
                .find(|p| p.name == active.name)
                .map(|p| p.duration_minutes)
                .unwrap_or(0);
            (active.started_at, dur)
        } else {
            let dur = m
                .phases
                .iter()
                .find(|p| p.name == instant.next.name)
                .map(|p| p.duration_minutes)
                .unwrap_or(0);
            (instant.next.starts_at, dur)
        };
        return Some(EventInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            map: m.map.clone(),
            expansion: m.expansion.clone().unwrap_or_else(|| "Core".into()),
            next_spawn: start_at,
            duration_minutes: duration,
            waypoint_code: m.waypoint_code.clone(),
        });
    }
    if let Some(lla) = &schedule.ley_line_anomaly {
        if lla.id == id {
            let pseudo = crate::timers::schedule::WorldBoss {
                id: lla.id.clone(),
                name: lla.name.clone(),
                tier: None,
                map: lla.rotation_maps.first().cloned().unwrap_or_default(),
                area: None,
                waypoint_code: None,
                expansion: "Core".into(),
                schedule_utc: lla.schedule_utc.clone(),
                duration_minutes: lla.duration_minutes,
                wiki_event: None,
            };
            return Some(EventInfo {
                id: lla.id.clone(),
                name: lla.name.clone(),
                map: lla.rotation_maps.join(" / "),
                expansion: "Special".into(),
                next_spawn: next_spawn(&pseudo, now),
                duration_minutes: lla.duration_minutes,
                waypoint_code: None,
            });
        }
    }
    None
}

fn load_pinned_achievement_views(
    db: &Db,
    upcoming: &[UpcomingEvent],
    weights: &Weights,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<PinnedItemView>> {
    let item_cache = load_item_cache_for_pinned(db)?;
    let rows = db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT pin.achievement_id, pin.collection_key,
                    a.name, a.description, a.requirement, COALESCE(a.points, 0),
                    a.bits,
                    p.current, p.max, COALESCE(p.done, 0), p.bits,
                    md.associated_boss, md.estimated_time_minutes
             FROM pinned_achievements pin
             LEFT JOIN achievements a ON a.id = pin.achievement_id
             LEFT JOIN account_progress p ON p.achievement_id = pin.achievement_id
             LEFT JOIN achievement_metadata md ON md.achievement_id = pin.achievement_id
             ORDER BY pin.pinned_at",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok(PinnedRow {
                id: r.get::<_, i64>(0)? as u32,
                collection_key: r.get(1)?,
                name: r.get::<_, Option<String>>(2)?,
                description: r.get(3)?,
                requirement: r.get(4)?,
                points: r.get(5)?,
                bits_def_json: r.get(6)?,
                current: r.get(7)?,
                max: r.get(8)?,
                done: r.get::<_, i64>(9)? != 0,
                bits_done_json: r.get(10)?,
                associated_boss: r.get(11)?,
                effort_minutes: r.get::<_, Option<i64>>(12)?.unwrap_or(30) as u32,
            })
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })?;

    let items: Vec<PinnedItemView> = rows
        .into_iter()
        .map(|r| {
            let ratio = match (r.current, r.max) {
                (Some(c), Some(m)) if m > 0 => (c as f64 / m as f64).clamp(0.0, 1.0),
                _ => {
                    if r.done {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
            let related: Vec<String> = r.associated_boss.clone().into_iter().collect();
            let scoreable = Scoreable {
                id: r.id.to_string(),
                completion_ratio: ratio,
                reward_value: r.points.max(0) as u32,
                effort_minutes: r.effort_minutes,
                related_event_ids: related.clone(),
            };
            let s = score_item(&scoreable, upcoming, weights, now);
            let next_event = r
                .associated_boss
                .as_ref()
                .and_then(|boss| upcoming.iter().find(|e| &e.id == boss).cloned());
            let bits = parse_bits(&r.bits_def_json, &r.bits_done_json, &item_cache);
            PinnedItemView {
                id: r.id,
                name: r.name.unwrap_or_else(|| format!("Achievement #{}", r.id)),
                description: r.description,
                requirement: r.requirement,
                current: r.current,
                max: r.max,
                completion_ratio: ratio,
                done: r.done,
                points: r.points,
                collection_key: r.collection_key,
                associated_boss: r.associated_boss,
                next_event,
                score: s,
                bits,
                is_pinned: true,
            }
        })
        .collect();
    Ok(items)
}

/// Load every achievement linked to a boss via achievement_metadata
/// (excluding ids the caller already has). Used by cmd_get_pinned_view to
/// surface the full done/to-do list for a pinned boss, not only the
/// achievements the user explicitly pinned.
fn load_boss_linked_achievements(
    db: &Db,
    boss_id: &str,
    upcoming: &[UpcomingEvent],
    weights: &Weights,
    now: chrono::DateTime<chrono::Utc>,
    exclude: &std::collections::HashSet<u32>,
) -> Result<Vec<PinnedItemView>> {
    let item_cache = load_item_cache_for_pinned(db)?;
    let rows = db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT a.id, a.name, a.description, a.requirement, COALESCE(a.points, 0),
                    a.bits,
                    p.current, p.max, COALESCE(p.done, 0), p.bits
             FROM achievement_metadata md
             LEFT JOIN achievements a ON a.id = md.achievement_id
             LEFT JOIN account_progress p ON p.achievement_id = md.achievement_id
             WHERE md.associated_boss = ?1 AND a.id IS NOT NULL
             ORDER BY a.id",
        )?;
        let mapped = stmt.query_map(rusqlite::params![boss_id], |r| {
            Ok((
                r.get::<_, i64>(0)? as u32,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, i64>(8)? != 0,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })?;

    let mut views = Vec::new();
    for (id, name, description, requirement, points, bits_def, current, max, done, bits_done) in rows {
        if exclude.contains(&id) {
            continue;
        }
        let ratio = match (current, max) {
            (Some(c), Some(m)) if m > 0 => (c as f64 / m as f64).clamp(0.0, 1.0),
            _ => {
                if done {
                    1.0
                } else {
                    0.0
                }
            }
        };
        let related: Vec<String> = vec![boss_id.to_string()];
        let scoreable = Scoreable {
            id: id.to_string(),
            completion_ratio: ratio,
            reward_value: points.max(0) as u32,
            effort_minutes: 30,
            related_event_ids: related,
        };
        let s = score_item(&scoreable, upcoming, weights, now);
        let next_event = upcoming.iter().find(|e| e.id == boss_id).cloned();
        let bits = parse_bits(&bits_def, &bits_done, &item_cache);
        views.push(PinnedItemView {
            id,
            name: name.unwrap_or_else(|| format!("Achievement #{id}")),
            description,
            requirement,
            current,
            max,
            completion_ratio: ratio,
            done,
            points,
            collection_key: None,
            associated_boss: Some(boss_id.to_string()),
            next_event,
            score: s,
            bits,
            is_pinned: false,
        });
    }
    Ok(views)
}

/// Collect every Item id referenced by a pinned achievement's bits and
/// pull its cached entry. Returns the lookup table consumed by `parse_bits`.
fn load_item_cache_for_pinned(
    db: &Db,
) -> Result<std::collections::HashMap<u32, crate::sync::items::CachedItem>> {
    let ids = db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT a.bits FROM pinned_achievements pin
             JOIN achievements a ON a.id = pin.achievement_id
             WHERE a.bits IS NOT NULL",
        )?;
        let mut rows = stmt.query([])?;
        let mut ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
        while let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&s) {
                for bit in arr {
                    let kind = bit.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if kind == "Item" {
                        if let Some(id) = bit.get("id").and_then(|v| v.as_u64()) {
                            ids.insert(id as u32);
                        }
                    }
                }
            }
        }
        Ok(ids.into_iter().collect::<Vec<_>>())
    })?;
    crate::sync::items::lookup_items(db, &ids)
}

/// Fetch any Item-typed bit referenced by pinned achievements that we don't
/// yet have in the items cache. Called by the frontend after a pin/unpin so
/// the next get_pinned_view shows real item names instead of `Item #X`.
///
/// Returns the number of items that were *requested* (not just the ones the
/// API actually returned) so the frontend can always know a warm pass
/// happened and trigger one final refresh.
#[tauri::command]
pub async fn cmd_warm_item_cache(state: State<'_, AppState>) -> Result<usize> {
    let missing = crate::sync::items::find_missing_item_ids(&state.db)?;
    if missing.is_empty() {
        tracing::debug!("warm_item_cache: nothing to fetch");
        return Ok(0);
    }
    tracing::info!(count = missing.len(), "warm_item_cache: fetching items");
    let key = load_api_key(&state.db)?.ok_or(AppError::NoApiKey)?;
    let client = ApiClient::new(Some(key))?;
    let fetched =
        crate::sync::items::fetch_and_cache_items(&client, &state.db, &missing).await?;
    tracing::info!(requested = missing.len(), fetched, "warm_item_cache: done");
    Ok(missing.len())
}

/// Parse the cached `achievements.bits` JSON array against the user's
/// `account_progress.bits` (list of completed indices) and return a flat
/// view ready for the UI. `item_cache` is consulted to resolve Item-typed
/// bits to human-readable names.
fn parse_bits(
    def_json: &Option<String>,
    done_json: &Option<String>,
    item_cache: &std::collections::HashMap<u32, crate::sync::items::CachedItem>,
) -> Vec<PinnedBitView> {
    let Some(def_str) = def_json else { return vec![] };
    let defs: Vec<serde_json::Value> = match serde_json::from_str(def_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let done_set: std::collections::HashSet<u32> = done_json
        .as_ref()
        .and_then(|s| serde_json::from_str::<Vec<u32>>(s).ok())
        .unwrap_or_default()
        .into_iter()
        .collect();

    defs.into_iter()
        .enumerate()
        .map(|(idx, bit)| {
            let kind = bit
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("Text")
                .to_string();
            let ref_id = bit.get("id").and_then(|v| v.as_i64());
            let text = bit
                .get("text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let (resolved_name, resolved_description, resolved_rarity) =
                if kind == "Item" {
                    ref_id
                        .and_then(|id| item_cache.get(&(id as u32)))
                        .map(|it| {
                            (
                                Some(it.name.clone()),
                                it.description.clone(),
                                it.rarity.clone(),
                            )
                        })
                        .unwrap_or((None, None, None))
                } else {
                    (None, None, None)
                };
            PinnedBitView {
                index: idx as u32,
                kind,
                ref_id,
                text,
                done: done_set.contains(&(idx as u32)),
                resolved_name,
                resolved_description,
                resolved_rarity,
            }
        })
        .collect()
}

struct PinnedRow {
    id: u32,
    collection_key: Option<String>,
    name: Option<String>,
    description: Option<String>,
    requirement: Option<String>,
    bits_def_json: Option<String>,
    bits_done_json: Option<String>,
    points: i64,
    current: Option<i64>,
    max: Option<i64>,
    done: bool,
    associated_boss: Option<String>,
    effort_minutes: u32,
}

fn read_latest_period(db: &Db, period_type: &str) -> Result<Option<WizardsVaultPeriodView>> {
    db.with_conn(|c| {
        // Find the most recent period_start for this period_type.
        let period_start: Option<String> = c
            .query_row(
                "SELECT MAX(period_start) FROM wizardsvault WHERE period_type = ?1",
                params![period_type],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);

        let Some(period_start) = period_start else {
            return Ok(None);
        };

        let mut stmt = c.prepare(
            "SELECT objective_id, title, track, acclaim, progress_current,
                    progress_complete, claimed
             FROM wizardsvault
             WHERE period_type = ?1 AND period_start = ?2
             ORDER BY objective_id",
        )?;
        let mapped = stmt.query_map(params![period_type, period_start], |r| {
            Ok(WizardsVaultObjectiveView {
                id: r.get::<_, i64>(0)? as u32,
                title: r.get(1)?,
                track: r.get(2)?,
                acclaim: r.get::<_, i64>(3)? as u32,
                progress_current: r.get::<_, i64>(4)? as u32,
                progress_complete: r.get::<_, i64>(5)? as u32,
                claimed: r.get::<_, i64>(6)? != 0,
            })
        })?;
        let mut objectives = Vec::new();
        for row in mapped {
            objectives.push(row?);
        }
        Ok(Some(WizardsVaultPeriodView {
            period_type: period_type.to_string(),
            period_start,
            objectives,
        }))
    })
}
