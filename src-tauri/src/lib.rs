pub mod api;
pub mod catalog;
pub mod db;
pub mod error;
pub mod scorer;
pub mod sync;
pub mod timers;
mod commands;

use std::fs;
use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::api::auth::load_api_key;
use crate::api::client::ApiClient;
use crate::commands::AppState;
use crate::db::repository::Db;
use crate::sync::engine::SyncEngine;
use crate::timers::schedule::Schedule;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,gw2_overlay_lib=debug")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::cmd_set_api_key,
            commands::cmd_check_api_key,
            commands::cmd_clear_api_key,
            commands::cmd_sync_now,
            commands::cmd_get_upcoming_events,
            commands::cmd_get_wizardsvault_state,
            commands::cmd_get_progress_summary,
            commands::cmd_search_achievements,
            commands::cmd_pin_achievement,
            commands::cmd_unpin_achievement,
            commands::cmd_list_legendary_collections,
            commands::cmd_get_pinned_view,
            commands::cmd_pin_boss,
            commands::cmd_unpin_boss,
            commands::cmd_remove_boss_group,
            commands::cmd_list_events,
        ])
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("no app data dir");
            fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("gw2-overlay.sqlite");
            info!(?db_path, "opening database");

            let db = Arc::new(Db::open(&db_path).map_err(|e| {
                error!(error = %e, "failed to open database");
                e
            })?);
            info!(achievements = db.count_achievements().unwrap_or(-1), "database ready");

            let schedule = Arc::new(Schedule::load().map_err(|e| {
                error!(error = %e, "failed to load embedded boss schedule");
                e
            })?);
            info!(
                bosses = schedule.world_bosses.len(),
                metas = schedule.meta_events.len(),
                "boss schedule loaded"
            );

            if let Err(e) = catalog::load_all(&db) {
                error!(error = %e, "failed to load static catalogs");
                return Err(Box::new(e));
            }

            // If a key is already stored, build the client + engine eagerly so
            // sync starts at boot. Otherwise the UI will prompt for one.
            let engine = match load_api_key(&db) {
                Ok(Some(key)) => match ApiClient::new(Some(key)) {
                    Ok(client) => {
                        let engine = SyncEngine::new(Arc::new(client), Arc::clone(&db));
                        engine.start();
                        info!("sync engine started with stored API key");
                        Some(engine)
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to build API client from stored key");
                        None
                    }
                },
                Ok(None) => {
                    info!("no stored API key, waiting for user to provide one");
                    None
                }
                Err(e) => {
                    warn!(error = %e, "failed to load stored API key");
                    None
                }
            };

            app.manage(AppState { db, engine: Mutex::new(engine), schedule });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
