use std::sync::Arc;

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

/// Lives in Tauri's `State<>`. `engine` is `None` until a valid key is set.
pub struct AppState {
    pub db: Arc<Db>,
    pub engine: Mutex<Option<SyncEngine>>,
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
