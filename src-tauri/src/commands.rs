use std::sync::Arc;

use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use tracing::info;

use crate::api::auth::{ApiKey, clear_api_key, load_api_key, store_api_key};
use crate::api::client::ApiClient;
use crate::api::endpoints::{self, TokenInfo};
use crate::db::repository::Db;
use crate::error::{AppError, Result};
use crate::sync::engine::SyncEngine;
use crate::sync::{progress, wizardsvault};
use crate::timers::engine::{UpcomingEvent, all_upcoming};
use crate::timers::schedule::Schedule;

/// Lives in Tauri's `State<>`. `engine` is `None` until a valid key is set.
pub struct AppState {
    pub db: Arc<Db>,
    pub engine: Mutex<Option<SyncEngine>>,
    pub schedule: Arc<Schedule>,
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

    let mut engine_guard = state.engine.lock().await;
    if let Some(prev) = engine_guard.take() {
        prev.shutdown();
    }
    let client = Arc::new(ApiClient::new(Some(parsed.clone()))?);
    let engine = SyncEngine::new(client, Arc::clone(&state.db));
    engine.start();
    *engine_guard = Some(engine);

    Ok(ApiKeyStatus::from(parsed.account_id().to_string(), info))
}

/// Returns the current key's status (None if no key configured).
#[tauri::command]
pub async fn cmd_check_api_key(state: State<'_, AppState>) -> Result<Option<ApiKeyStatus>> {
    let Some(key) = load_api_key(&state.db)? else {
        return Ok(None);
    };
    let probe = ApiClient::new(Some(key.clone()))?;
    let info = endpoints::get_tokeninfo(&probe).await?;
    Ok(Some(ApiKeyStatus::from(key.account_id().to_string(), info)))
}

#[tauri::command]
pub async fn cmd_clear_api_key(state: State<'_, AppState>) -> Result<()> {
    let mut engine_guard = state.engine.lock().await;
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
    let engine_guard = state.engine.lock().await;
    let Some(engine) = engine_guard.as_ref() else {
        return Err(AppError::NoApiKey);
    };

    let key = load_api_key(&state.db)?.ok_or(AppError::NoApiKey)?;
    let client = ApiClient::new(Some(key))?;
    let snapshot = engine.snapshot();

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
pub async fn cmd_get_upcoming_events(
    state: State<'_, AppState>,
    horizon_minutes: i64,
) -> Result<Vec<UpcomingEvent>> {
    Ok(all_upcoming(&state.schedule, Utc::now(), horizon_minutes))
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
