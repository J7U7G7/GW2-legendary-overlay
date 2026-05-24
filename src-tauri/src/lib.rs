mod api;
mod commands;
mod db;
mod error;
mod scorer;
mod sync;
mod timers;

use std::fs;

use tauri::Manager;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::db::repository::Db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,gw2_overlay_lib=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("no app data dir");
            fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("gw2-overlay.sqlite");
            info!(?db_path, "opening database");

            let db = match Db::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    error!(error = %e, "failed to open database");
                    return Err(Box::new(e));
                }
            };
            info!(achievements = db.count_achievements().unwrap_or(-1), "database ready");
            app.manage(db);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
