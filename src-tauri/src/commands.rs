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

#[derive(Serialize)]
pub struct PinnedItemView {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub current: Option<i64>,
    pub max: Option<i64>,
    pub completion_ratio: f64,
    pub done: bool,
    pub points: i64,
    pub collection_key: Option<String>,
    pub associated_boss: Option<String>,
    pub next_event: Option<UpcomingEvent>,
    pub score: f64,
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
    state.db.pin_achievement(achievement_id, collection_key.as_deref())
}

#[tauri::command]
pub async fn cmd_unpin_achievement(
    state: State<'_, AppState>,
    achievement_id: u32,
) -> Result<()> {
    state.db.unpin_achievement(achievement_id)
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
pub async fn cmd_get_pinned_view(state: State<'_, AppState>) -> Result<Vec<PinnedItemView>> {
    let now = chrono::Utc::now();
    let upcoming = all_upcoming(&state.schedule, now, 240);
    let weights = Weights::default();

    let rows = state.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT pin.achievement_id, pin.collection_key,
                    a.name, a.description, COALESCE(a.points, 0),
                    p.current, p.max, COALESCE(p.done, 0),
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
                points: r.get(4)?,
                current: r.get(5)?,
                max: r.get(6)?,
                done: r.get::<_, i64>(7)? != 0,
                associated_boss: r.get(8)?,
                effort_minutes: r.get::<_, Option<i64>>(9)?.unwrap_or(30) as u32,
            })
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })?;

    let mut scored: Vec<PinnedItemView> = rows
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
            let related: Vec<String> = r.associated_boss.iter().cloned().collect();
            let scoreable = Scoreable {
                id: r.id.to_string(),
                completion_ratio: ratio,
                reward_value: r.points.max(0) as u32,
                effort_minutes: r.effort_minutes,
                related_event_ids: related.clone(),
            };
            let s = score_item(&scoreable, &upcoming, &weights, now);
            let next_event = r
                .associated_boss
                .as_ref()
                .and_then(|boss| upcoming.iter().find(|e| &e.id == boss).cloned());
            PinnedItemView {
                id: r.id,
                name: r.name.unwrap_or_else(|| format!("Achievement #{}", r.id)),
                description: r.description,
                current: r.current,
                max: r.max,
                completion_ratio: ratio,
                done: r.done,
                points: r.points,
                collection_key: r.collection_key,
                associated_boss: r.associated_boss,
                next_event,
                score: s,
            }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored)
}

struct PinnedRow {
    id: u32,
    collection_key: Option<String>,
    name: Option<String>,
    description: Option<String>,
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
