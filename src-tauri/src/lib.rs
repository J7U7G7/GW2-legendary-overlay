pub mod api;
pub mod builds;
pub mod catalog;
pub mod db;
pub mod error;
pub mod legendary;
pub mod scorer;
pub mod sync;
pub mod timers;
mod commands;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use tauri_plugin_window_state::{StateFlags, WindowExt as _};

use crate::api::auth::load_api_key;
use crate::api::client::ApiClient;
use crate::commands::AppState;
use crate::db::repository::Db;
use crate::sync::engine::SyncEngine;
use crate::timers::schedule::Schedule;

/// Resolve the logs directory using the same convention Tauri uses for
/// `app_data_dir`. We need this *before* the Tauri builder runs so logging
/// is wired up for the earliest startup code paths (catalog seeding, schema
/// migration, etc.).
fn resolve_logs_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("com.tripleseptconsulting.gw2overlay").join("logs")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let logs_dir = resolve_logs_dir();
    let _ = fs::create_dir_all(&logs_dir);
    // Daily rotation. Files look like `gw2-overlay.log.2026-05-25`. We let
    // the appender accumulate (no automatic purge — keep the last few weeks
    // around for after-the-fact diagnosis). User can wipe via the
    // 'Open logs folder' button in Settings.
    let file_appender = tracing_appender::rolling::daily(&logs_dir, "gw2-overlay.log");
    // Non-blocking writer keeps a guard alive for the duration of the
    // process; dropping the guard would flush+drop pending writes.
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
    // Leak the guard so logs keep flowing for the whole process lifetime.
    // Storing it in a static would be cleaner, but `Box::leak` is fine for a
    // single-instance desktop app.
    Box::leak(Box::new(file_guard));

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,gw2_overlay_lib=debug"));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(true),
        )
        .try_init();

    // Convert Rust panics into structured log entries. Tauri otherwise
    // swallows panics from background tasks silently — this hook ensures any
    // panic in the sync engine, the timer engine, or the IPC handlers lands
    // in the log file the user can attach to a bug report.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        tracing::error!(location = %location, panic = %payload, "PANIC");
        prev_hook(info);
    }));

    info!(
        logs_dir = %logs_dir.display(),
        version = env!("CARGO_PKG_VERSION"),
        "gw2-overlay starting"
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_window_event(|window, event| {
            // Secondary windows (bosses + achievements) hide on close
            // instead of quitting the app. Main window keeps default
            // close-quits behavior.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    "bosses" | "achievements" => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_set_api_key,
            commands::cmd_check_api_key,
            commands::cmd_clear_api_key,
            commands::cmd_sync_now,
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
            commands::cmd_warm_item_cache,
            commands::cmd_get_appearance,
            commands::cmd_set_appearance,
            commands::cmd_save_state_and_quit,
            commands::cmd_test_notification,
            commands::cmd_get_notification_lead,
            commands::cmd_set_notification_lead,
            commands::cmd_get_hotkeys,
            commands::cmd_set_hotkeys,
            commands::cmd_sync_account_items,
            commands::cmd_search_account_items,
            commands::cmd_sync_wallet,
            commands::cmd_search_currencies,
            commands::cmd_list_todos,
            commands::cmd_add_todo,
            commands::cmd_toggle_todo,
            commands::cmd_delete_todo,
            commands::cmd_list_builds,
            commands::cmd_legendary_progress,
            commands::cmd_reset_database,
            commands::cmd_open_logs_folder,
            commands::cmd_recent_logs,
            commands::cmd_app_version,
            commands::cmd_reset_window_layout,
            commands::cmd_log_event,
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

            // Manage state EARLY so the secondary windows' JS can invoke
            // commands the moment their webview boots. The engine itself
            // is wired in below — it can take a moment (ApiClient build,
            // tokio spawn) and we don't want to keep the windows blocked
            // on it.
            app.manage(AppState {
                db: Arc::clone(&db),
                engine: Mutex::new(None),
                schedule: Arc::clone(&schedule),
                app_handle: app.handle().clone(),
            });

            // Restore window position/size for every window. Plugin
            // declares the save handlers; we still trigger the restore
            // explicitly because tauri.conf.json declares default sizes.
            for label in ["main", "bosses", "achievements"] {
                if let Some(window) = app.get_webview_window(label) {
                    if let Err(e) = window.restore_state(StateFlags::all()) {
                        warn!(label, error = %e, "window state restore failed");
                    }
                }
            }

            // If a key is already stored, build the client + engine eagerly
            // so sync starts at boot. Otherwise the UI will prompt for one.
            if let Ok(Some(key)) = load_api_key(&db) {
                match ApiClient::new(Some(key)) {
                    Ok(client) => {
                        let engine = SyncEngine::new(
                            Arc::new(client),
                            Arc::clone(&db),
                            Arc::clone(&schedule),
                            app.handle().clone(),
                        );
                        engine.start();
                        info!("sync engine started with stored API key");
                        let state: tauri::State<AppState> = app.state();
                        *state.engine.lock().expect("engine mutex poisoned") = Some(engine);
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to build API client from stored key");
                    }
                }
            } else {
                info!("no stored API key, waiting for user to provide one");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
